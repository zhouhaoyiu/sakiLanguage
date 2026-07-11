use crate::seismic::{
    ElasticMedium, FaultGeometry, bandpass_zero_phase, homogeneous_greens, invert_finite_fault,
    invert_moment_tensor, read_miniseed,
};
use crate::value::Value;
use std::cell::RefCell;
use std::collections::HashMap;
use std::fs;
use std::rc::Rc;

#[derive(Debug, Clone)]
struct Binding {
    value: Value,
    mutable: bool,
}

#[derive(Debug, Clone)]
struct EnvironmentData {
    variables: HashMap<String, Binding>,
    parent: Option<Environment>,
    function_scope: bool,
}

#[derive(Debug, Clone)]
/// 运行时变量作用域。
pub struct Environment {
    inner: Rc<RefCell<EnvironmentData>>,
}

impl Environment {
    /// 创建顶层作用域并注册内建函数。
    pub fn new() -> Self {
        let env = Environment {
            inner: Rc::new(RefCell::new(EnvironmentData {
                variables: HashMap::new(),
                parent: None,
                function_scope: true,
            })),
        };
        env.define_with("saki", Value::NativeFunction(native_saki), false);
        env.define_with("null", Value::Null, false);
        env.define_with("undefined", Value::Undefined, false);
        env.define_with(
            "read_waveform",
            Value::NativeFunction(native_read_waveform),
            false,
        );
        env.define_with("bandpass", Value::NativeFunction(native_bandpass), false);
        env.define_with("window", Value::NativeFunction(native_window), false);
        env.define_with("pick", Value::NativeFunction(native_pick), false);
        env.define_with(
            "ground_motion",
            Value::NativeFunction(native_ground_motion),
            false,
        );
        env.define_with("qc", Value::NativeFunction(native_qc), false);
        env.define_with(
            "source_inversion",
            Value::NativeFunction(native_source_inversion),
            false,
        );
        env.define_with(
            "green_functions",
            Value::NativeFunction(native_green_functions),
            false,
        );
        env.define_with(
            "finite_fault_inversion",
            Value::NativeFunction(native_finite_fault_inversion),
            false,
        );
        env.define_with("export", Value::NativeFunction(native_export), false);
        env
    }

    /// 创建块作用域。
    pub fn new_enclosed(parent: Environment) -> Self {
        Self::new_child(parent, false)
    }

    /// 创建函数作用域。
    pub fn new_function(parent: Environment) -> Self {
        Self::new_child(parent, true)
    }

    fn new_child(parent: Environment, function_scope: bool) -> Self {
        Environment {
            inner: Rc::new(RefCell::new(EnvironmentData {
                variables: HashMap::new(),
                parent: Some(parent),
                function_scope,
            })),
        }
    }

    /// 在作用域链中查找变量。
    pub fn get(&self, name: &str) -> Result<Value, String> {
        let mut env = Some(self.clone());
        while let Some(current) = env {
            let data = current.inner.borrow();
            if let Some(binding) = data.variables.get(name) {
                return Ok(binding.value.clone());
            }
            env = data.parent.clone();
        }
        Err(format!("未定义的变量 '{}'", name))
    }

    /// 变量赋值。
    pub fn set(&self, name: &str, value: Value) -> Result<(), String> {
        let mut env = Some(self.clone());
        while let Some(current) = env {
            let mut data = current.inner.borrow_mut();
            if let Some(binding) = data.variables.get_mut(name) {
                if !binding.mutable {
                    return Err(format!("变量 '{}' 是只读变量", name));
                }
                binding.value = value;
                return Ok(());
            }
            env = data.parent.clone();
        }
        Err(format!("未定义的变量 '{}'", name))
    }

    /// 在当前作用域中定义可写变量。
    pub fn define(&self, name: &str, value: Value) {
        self.define_with(name, value, true);
    }

    /// 在当前作用域中定义变量，并设置可变性。
    pub fn define_with(&self, name: &str, value: Value, mutable: bool) {
        self.inner
            .borrow_mut()
            .variables
            .insert(name.to_string(), Binding { value, mutable });
    }

    /// var 进入最近的函数/全局作用域。
    pub fn define_var(&self, name: &str, value: Value) {
        let mut target = self.clone();
        loop {
            let parent = {
                let data = target.inner.borrow();
                if data.function_scope {
                    None
                } else {
                    data.parent.clone()
                }
            };

            if let Some(parent) = parent {
                target = parent;
            } else {
                target.define(name, value);
                return;
            }
        }
    }
}

/// 内建输出函数：打印参数并换行。
fn native_saki(args: &[Value]) -> Result<Value, String> {
    for val in args {
        print!("{}", val);
    }
    println!();
    Ok(Value::Null)
}

fn expect_str<'a>(args: &'a [Value], index: usize, name: &str) -> Result<&'a str, String> {
    match args.get(index) {
        Some(Value::Str(value)) => Ok(value),
        _ => Err(format!("{} 的第 {} 个参数必须是字符串", name, index + 1)),
    }
}

fn expect_number(args: &[Value], index: usize, name: &str) -> Result<f64, String> {
    match args.get(index) {
        Some(Value::Int(value)) => Ok(*value as f64),
        Some(Value::Float(value)) => Ok(*value),
        _ => Err(format!("{} 的第 {} 个参数必须是数值", name, index + 1)),
    }
}

fn numeric_array(value: &Value, name: &str) -> Result<Vec<f64>, String> {
    let Value::Array(items) = value else {
        return Err(format!("{} 必须是数值数组", name));
    };
    items
        .iter()
        .map(|item| match item {
            Value::Int(value) => Ok(*value as f64),
            Value::Float(value) if value.is_finite() => Ok(*value),
            _ => Err(format!("{} 包含非数值或非有限值", name)),
        })
        .collect()
}

fn numeric_matrix(value: &Value, name: &str) -> Result<Vec<Vec<f64>>, String> {
    let Value::Array(rows) = value else {
        return Err(format!("{} 必须是二维数值数组", name));
    };
    rows.iter()
        .enumerate()
        .map(|(index, row)| numeric_array(row, &format!("{}[{}]", name, index)))
        .collect()
}

fn waveform_data(waveform: &Value, name: &str) -> Result<(Vec<f64>, f64), String> {
    let samples = waveform
        .property("samples")
        .ok_or_else(|| format!("{} 的 waveform 缺少 samples", name))?;
    let sampling_rate = waveform
        .property("sampling_rate_hz")
        .ok_or_else(|| format!("{} 的 waveform 缺少 sampling_rate_hz", name))?;
    let samples = numeric_array(samples, "waveform.samples")?;
    let sampling_rate = match sampling_rate {
        Value::Int(value) => *value as f64,
        Value::Float(value) => *value,
        _ => return Err(format!("{} 的 sampling_rate_hz 必须是数值", name)),
    };
    Ok((samples, sampling_rate))
}

fn numbers(values: Vec<f64>) -> Value {
    Value::Array(values.into_iter().map(Value::Float).collect())
}

fn matrix(values: Vec<Vec<f64>>) -> Value {
    Value::Array(values.into_iter().map(numbers).collect())
}

fn vector3(value: &Value, name: &str) -> Result<[f64; 3], String> {
    let values = numeric_array(value, name)?;
    values
        .try_into()
        .map_err(|_| format!("{} 必须包含 3 个坐标", name))
}

fn vectors3(value: &Value, name: &str) -> Result<Vec<[f64; 3]>, String> {
    let Value::Array(items) = value else {
        return Err(format!("{} 必须是坐标数组", name));
    };
    items
        .iter()
        .enumerate()
        .map(|(index, value)| vector3(value, &format!("{}[{}]", name, index)))
        .collect()
}

fn object_number(value: &Value, key: &str, name: &str) -> Result<f64, String> {
    match value.property(key) {
        Some(Value::Int(value)) => Ok(*value as f64),
        Some(Value::Float(value)) => Ok(*value),
        _ => Err(format!("{}.{} 必须是数值", name, key)),
    }
}

fn object_usize(value: &Value, key: &str, name: &str) -> Result<usize, String> {
    match value.property(key) {
        Some(Value::Int(value)) if *value > 0 => Ok(*value as usize),
        _ => Err(format!("{}.{} 必须是正整数", name, key)),
    }
}

fn medium(value: &Value) -> Result<ElasticMedium, String> {
    Ok(ElasticMedium {
        vp_km_s: object_number(value, "vp_km_s", "medium")?,
        vs_km_s: object_number(value, "vs_km_s", "medium")?,
        density_kg_m3: object_number(value, "density_kg_m3", "medium")?,
    })
}

fn fault(value: &Value) -> Result<FaultGeometry, String> {
    Ok(FaultGeometry {
        length_km: object_number(value, "length_km", "fault")?,
        width_km: object_number(value, "width_km", "fault")?,
        strike_deg: object_number(value, "strike_deg", "fault")?,
        dip_deg: object_number(value, "dip_deg", "fault")?,
        rake_deg: object_number(value, "rake_deg", "fault")?,
        nstrike: object_usize(value, "nstrike", "fault")?,
        ndip: object_usize(value, "ndip", "fault")?,
    })
}

fn expect_waveform(args: &[Value], index: usize, name: &str) -> Result<Value, String> {
    let value = args
        .get(index)
        .ok_or_else(|| format!("{} 缺少第 {} 个参数", name, index + 1))?;
    match value.property("kind") {
        Some(Value::Str(kind)) if kind == "waveform" => Ok(value.clone()),
        _ => Err(format!("{} 的第 {} 个参数必须是 waveform", name, index + 1)),
    }
}

fn native_read_waveform(args: &[Value]) -> Result<Value, String> {
    if args.len() != 1 {
        return Err("read_waveform 需要 1 个参数: path".to_string());
    }
    let path = expect_str(args, 0, "read_waveform")?;
    let waveform = read_miniseed(path)?;
    Ok(Value::object(vec![
        ("kind", Value::Str("waveform".to_string())),
        ("path", Value::Str(path.to_string())),
        ("format", Value::Str(waveform.format)),
        ("source_identifier", Value::Str(waveform.source_identifier)),
        ("extra_headers", Value::Str(waveform.extra_headers)),
        ("network", Value::Str(waveform.network)),
        ("station", Value::Str(waveform.station)),
        ("location", Value::Str(waveform.location)),
        ("channel", Value::Str(waveform.channel)),
        ("start_time", Value::Str(waveform.start_time)),
        ("samples", numbers(waveform.samples)),
        ("sampling_rate_hz", Value::Float(waveform.sampling_rate_hz)),
        ("unit", Value::Str("counts".to_string())),
        (
            "provenance",
            Value::Str(format!("read_miniseed_v2({})", path)),
        ),
    ]))
}

fn native_bandpass(args: &[Value]) -> Result<Value, String> {
    if args.len() != 3 {
        return Err("bandpass 需要 3 个参数: waveform, low_hz, high_hz".to_string());
    }
    let waveform = expect_waveform(args, 0, "bandpass")?;
    let low = expect_number(args, 1, "bandpass")?;
    let high = expect_number(args, 2, "bandpass")?;
    let (samples, sampling_rate) = waveform_data(&waveform, "bandpass")?;
    let filtered = bandpass_zero_phase(&samples, sampling_rate, low, high)?;
    Ok(Value::object(vec![
        ("kind", Value::Str("waveform".to_string())),
        ("source", waveform),
        ("operation", Value::Str("bandpass".to_string())),
        ("samples", numbers(filtered)),
        ("sampling_rate_hz", Value::Float(sampling_rate)),
        ("low_hz", Value::Float(low)),
        ("high_hz", Value::Float(high)),
        ("prototype_order", Value::Int(4)),
        ("zero_phase", Value::Bool(true)),
        (
            "provenance",
            Value::Str(format!(
                "butterworth_filtfilt(low_hz={}, high_hz={})",
                low, high
            )),
        ),
    ]))
}

fn native_window(args: &[Value]) -> Result<Value, String> {
    if args.len() != 3 {
        return Err("window 需要 3 个参数: waveform, start, end".to_string());
    }
    let waveform = expect_waveform(args, 0, "window")?;
    let start = expect_number(args, 1, "window")?;
    let end = expect_number(args, 2, "window")?;
    let (samples, sampling_rate) = waveform_data(&waveform, "window")?;
    if start < 0.0 || end <= start || end > samples.len() as f64 / sampling_rate {
        return Err("window 要求 0 <= start_s < end_s <= waveform duration".to_string());
    }
    let start_index = (start * sampling_rate).floor() as usize;
    let end_index = ((end * sampling_rate).ceil() as usize).min(samples.len());
    let unit = match waveform.property("unit") {
        Some(Value::Str(value)) => value.clone(),
        _ => "input-unit".to_string(),
    };
    Ok(Value::object(vec![
        ("kind", Value::Str("waveform".to_string())),
        ("source", waveform),
        ("operation", Value::Str("window".to_string())),
        ("samples", numbers(samples[start_index..end_index].to_vec())),
        ("sampling_rate_hz", Value::Float(sampling_rate)),
        ("unit", Value::Str(unit)),
        ("start_s", Value::Float(start)),
        ("end_s", Value::Float(end)),
        (
            "provenance",
            Value::Str(format!("window(start_s={}, end_s={})", start, end)),
        ),
    ]))
}

fn native_pick(args: &[Value]) -> Result<Value, String> {
    if !(1..=2).contains(&args.len()) {
        return Err("pick 需要 1-2 个参数: waveform, phase".to_string());
    }
    let waveform = expect_waveform(args, 0, "pick")?;
    let phase = if args.len() == 2 {
        expect_str(args, 1, "pick")?
    } else {
        "P"
    };
    Ok(Value::object(vec![
        ("kind", Value::Str("pick".to_string())),
        ("phase", Value::Str(phase.to_string())),
        ("time", Value::Undefined),
        ("confidence", Value::Undefined),
        ("source", waveform),
        ("provenance", Value::Str(format!("pick(phase={})", phase))),
    ]))
}

fn native_ground_motion(args: &[Value]) -> Result<Value, String> {
    if args.len() != 2 {
        return Err("ground_motion 需要 2 个参数: waveform, metric".to_string());
    }
    let waveform = expect_waveform(args, 0, "ground_motion")?;
    let metric = expect_str(args, 1, "ground_motion")?;
    if !matches!(metric, "PGA" | "PGV") {
        return Err("ground_motion 当前支持 PGA 或 PGV".to_string());
    }
    let (samples, _) = waveform_data(&waveform, "ground_motion")?;
    let peak = samples.iter().map(|value| value.abs()).fold(0.0, f64::max);
    let unit = match waveform.property("unit") {
        Some(Value::Str(unit)) => unit.clone(),
        _ => "input-unit".to_string(),
    };
    Ok(Value::object(vec![
        ("kind", Value::Str("ground_motion".to_string())),
        ("metric", Value::Str(metric.to_string())),
        ("value", Value::Float(peak)),
        ("unit", Value::Str(unit)),
        ("source", waveform),
        (
            "provenance",
            Value::Str(format!("ground_motion(metric={})", metric)),
        ),
    ]))
}

fn native_qc(args: &[Value]) -> Result<Value, String> {
    if args.len() != 1 {
        return Err("qc 需要 1 个参数: waveform".to_string());
    }
    let waveform = expect_waveform(args, 0, "qc")?;
    let (samples, sampling_rate) = waveform_data(&waveform, "qc")?;
    let mut issues = Vec::new();
    if samples.is_empty() {
        issues.push(Value::Str("empty waveform".to_string()));
    }
    if !sampling_rate.is_finite() || sampling_rate <= 0.0 {
        issues.push(Value::Str("invalid sampling rate".to_string()));
    }
    if samples.windows(2).all(|pair| pair[0] == pair[1]) && samples.len() > 1 {
        issues.push(Value::Str("constant waveform".to_string()));
    }
    Ok(Value::object(vec![
        ("kind", Value::Str("qc_report".to_string())),
        ("ok", Value::Bool(issues.is_empty())),
        ("issues", Value::Array(issues)),
        ("source", waveform),
        (
            "provenance",
            Value::Str("qc(samples, sampling_rate)".to_string()),
        ),
    ]))
}

fn native_source_inversion(args: &[Value]) -> Result<Value, String> {
    if !(2..=3).contains(&args.len()) {
        return Err("source_inversion 需要 2-3 个参数: greens, observations, damping".to_string());
    }
    let greens = numeric_matrix(&args[0], "greens")?;
    let observations = numeric_array(&args[1], "observations")?;
    let damping = if args.len() == 3 {
        expect_number(args, 2, "source_inversion")?
    } else {
        0.0
    };
    let result = invert_moment_tensor(&greens, &observations, damping)?;
    Ok(Value::object(vec![
        ("kind", Value::Str("source_inversion".to_string())),
        (
            "model",
            Value::Str("linear_full_moment_tensor_6".to_string()),
        ),
        (
            "components",
            Value::Array(vec![
                Value::Str("Mxx".to_string()),
                Value::Str("Myy".to_string()),
                Value::Str("Mzz".to_string()),
                Value::Str("Mxy".to_string()),
                Value::Str("Mxz".to_string()),
                Value::Str("Myz".to_string()),
            ]),
        ),
        ("moment_tensor", numbers(result.moment_tensor)),
        ("predicted", numbers(result.predicted)),
        ("residuals", numbers(result.residuals)),
        ("rms", Value::Float(result.rms)),
        (
            "variance_reduction",
            Value::Float(result.variance_reduction),
        ),
        ("condition_proxy", Value::Float(result.condition_proxy)),
        ("damping", Value::Float(damping)),
        ("units", Value::Str("input-defined".to_string())),
        (
            "provenance",
            Value::Str("damped_least_squares(G,d)".to_string()),
        ),
    ]))
}

fn native_green_functions(args: &[Value]) -> Result<Value, String> {
    if args.len() != 4 {
        return Err("green_functions 需要 4 个参数: stations, source, medium, phase".to_string());
    }
    let stations = vectors3(&args[0], "stations")?;
    let source = vector3(&args[1], "source")?;
    let medium = medium(&args[2])?;
    let phase = expect_str(args, 3, "green_functions")?;
    let result = homogeneous_greens(&stations, source, medium, phase)?;
    Ok(Value::object(vec![
        ("kind", Value::Str("green_functions".to_string())),
        (
            "model",
            Value::Str("homogeneous_isotropic_full_space_far_field".to_string()),
        ),
        ("phase", Value::Str(phase.to_string())),
        ("matrix", matrix(result.matrix)),
        ("travel_times_s", numbers(result.travel_times_s)),
        (
            "coordinates",
            Value::Str("x=east_km,y=north_km,z=down_km".to_string()),
        ),
        (
            "provenance",
            Value::Str("analytical_far_field_body_wave_kernel".to_string()),
        ),
    ]))
}

fn native_finite_fault_inversion(args: &[Value]) -> Result<Value, String> {
    if !(5..=9).contains(&args.len()) {
        return Err("finite_fault_inversion 需要 5-9 个参数: stations, observations, source, fault, medium, phase, damping, smoothing, iterations".to_string());
    }
    let stations = vectors3(&args[0], "stations")?;
    let observations = numeric_array(&args[1], "observations")?;
    let source = vector3(&args[2], "source")?;
    let initial = fault(&args[3])?;
    let medium = medium(&args[4])?;
    let phase = if args.len() >= 6 {
        expect_str(args, 5, "finite_fault_inversion")?
    } else {
        "S"
    };
    let damping = if args.len() >= 7 {
        expect_number(args, 6, "finite_fault_inversion")?
    } else {
        0.0
    };
    let smoothing = if args.len() >= 8 {
        expect_number(args, 7, "finite_fault_inversion")?
    } else {
        0.0
    };
    let iterations = if args.len() >= 9 {
        match args.get(8) {
            Some(Value::Int(value)) if *value > 0 => *value as usize,
            _ => return Err("iterations 必须是正整数".to_string()),
        }
    } else {
        500
    };
    let result = invert_finite_fault(
        &stations,
        &observations,
        source,
        initial,
        medium,
        phase,
        damping,
        smoothing,
        iterations,
    )?;
    let centers = result
        .patch_centers_km
        .into_iter()
        .map(|center| numbers(center.to_vec()))
        .collect();
    Ok(Value::object(vec![
        ("kind", Value::Str("finite_fault_inversion".to_string())),
        ("phase", Value::Str(phase.to_string())),
        ("strike_deg", Value::Float(result.geometry.strike_deg)),
        ("dip_deg", Value::Float(result.geometry.dip_deg)),
        ("rake_deg", Value::Float(result.geometry.rake_deg)),
        ("patch_centers_km", Value::Array(centers)),
        ("patch_moments_nm", numbers(result.patch_moments_nm)),
        ("patch_slips_m", numbers(result.patch_slips_m)),
        ("predicted", numbers(result.predicted)),
        ("residuals", numbers(result.residuals)),
        ("rms", Value::Float(result.rms)),
        (
            "variance_reduction",
            Value::Float(result.variance_reduction),
        ),
        ("objective", Value::Float(result.objective)),
        (
            "model_evaluations",
            Value::Int(result.model_evaluations as i64),
        ),
        (
            "provenance",
            Value::Str(
                "nonnegative_patch_moments+regularization+angle_coordinate_search".to_string(),
            ),
        ),
    ]))
}

fn native_export(args: &[Value]) -> Result<Value, String> {
    if args.len() != 2 {
        return Err("export 需要 2 个参数: value, path".to_string());
    }
    let path = expect_str(args, 1, "export")?;
    fs::write(path, args[0].to_string()).map_err(|err| format!("export 写入失败: {}", err))?;
    Ok(Value::object(vec![
        ("kind", Value::Str("export".to_string())),
        ("path", Value::Str(path.to_string())),
        ("bytes", Value::Int(args[0].to_string().len() as i64)),
    ]))
}
