//! 地震波形解析、滤波与线性矩张量反演内核。

use std::f64::consts::PI;
use std::fs;

#[derive(Debug, Clone, PartialEq)]
pub struct Waveform {
    pub samples: Vec<f64>,
    pub sampling_rate_hz: f64,
    pub format: String,
    pub source_identifier: String,
    pub extra_headers: String,
    pub network: String,
    pub station: String,
    pub location: String,
    pub channel: String,
    pub start_time: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct InversionResult {
    pub moment_tensor: Vec<f64>,
    pub predicted: Vec<f64>,
    pub residuals: Vec<f64>,
    pub rms: f64,
    pub variance_reduction: f64,
    pub condition_proxy: f64,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ElasticMedium {
    pub vp_km_s: f64,
    pub vs_km_s: f64,
    pub density_kg_m3: f64,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FaultGeometry {
    pub length_km: f64,
    pub width_km: f64,
    pub strike_deg: f64,
    pub dip_deg: f64,
    pub rake_deg: f64,
    pub nstrike: usize,
    pub ndip: usize,
}

#[derive(Debug, Clone, PartialEq)]
pub struct GreensResult {
    pub matrix: Vec<Vec<f64>>,
    pub travel_times_s: Vec<f64>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct FiniteFaultResult {
    pub geometry: FaultGeometry,
    pub patch_centers_km: Vec<[f64; 3]>,
    pub patch_moments_nm: Vec<f64>,
    pub patch_slips_m: Vec<f64>,
    pub predicted: Vec<f64>,
    pub residuals: Vec<f64>,
    pub rms: f64,
    pub variance_reduction: f64,
    pub objective: f64,
    pub model_evaluations: usize,
}

#[derive(Clone, Copy)]
enum ByteOrder {
    Big,
    Little,
}

#[derive(Clone, Copy)]
struct Biquad {
    b0: f64,
    b1: f64,
    b2: f64,
    a1: f64,
    a2: f64,
}

impl Biquad {
    fn apply(self, samples: &mut [f64]) {
        let (mut x1, mut x2, mut y1, mut y2) = (0.0, 0.0, 0.0, 0.0);
        for sample in samples {
            let x = *sample;
            let y = self.b0 * x + self.b1 * x1 + self.b2 * x2 - self.a1 * y1 - self.a2 * y2;
            *sample = y;
            (x2, x1, y2, y1) = (x1, x, y1, y);
        }
    }
}

/// 读取 MiniSEED 2 数据记录并合并同一连续数据流。
pub fn read_miniseed(path: &str) -> Result<Waveform, String> {
    let bytes = fs::read(path).map_err(|error| format!("读取 MiniSEED 失败: {}", error))?;
    parse_miniseed(&bytes)
}

/// 解析 MiniSEED 2，支持整数、IEEE 浮点、STEIM-1 和 STEIM-2 编码。
pub fn parse_miniseed(bytes: &[u8]) -> Result<Waveform, String> {
    if bytes.starts_with(b"MS") && bytes.get(2) == Some(&3) {
        return parse_miniseed3(bytes);
    }
    if bytes.len() < 64 {
        return Err("MiniSEED 文件短于最小记录头".to_string());
    }

    let mut offset = 0;
    let mut result: Option<Waveform> = None;
    while offset < bytes.len() {
        let remaining = &bytes[offset..];
        if remaining.len() < 64 {
            return Err(format!(
                "MiniSEED 尾部存在 {} 个不完整字节",
                remaining.len()
            ));
        }
        let record = parse_record(remaining)?;
        if offset + record.record_length > bytes.len() {
            return Err("MiniSEED 记录长度超过文件边界".to_string());
        }
        let data = &remaining[..record.record_length];
        let samples = decode_samples(
            &data[record.data_offset..],
            record.encoding,
            record.byte_order,
            record.sample_count,
        )?;

        if let Some(waveform) = &mut result {
            if waveform.network != record.network
                || waveform.station != record.station
                || waveform.location != record.location
                || waveform.channel != record.channel
            {
                return Err("一个 read_waveform 调用只能读取单一数据流".to_string());
            }
            if (waveform.sampling_rate_hz - record.sampling_rate_hz).abs() > 1e-9 {
                return Err("MiniSEED 记录采样率不一致".to_string());
            }
            waveform.samples.extend(samples);
        } else {
            result = Some(Waveform {
                samples,
                sampling_rate_hz: record.sampling_rate_hz,
                format: "MiniSEED 2".to_string(),
                source_identifier: format!(
                    "FDSN:{}_{}_{}_{}",
                    record.network, record.station, record.location, record.channel
                ),
                extra_headers: String::new(),
                network: record.network,
                station: record.station,
                location: record.location,
                channel: record.channel,
                start_time: record.start_time,
            });
        }
        offset += record.record_length;
    }
    result.ok_or_else(|| "MiniSEED 文件没有数据记录".to_string())
}

fn parse_miniseed3(bytes: &[u8]) -> Result<Waveform, String> {
    let mut offset = 0usize;
    let mut result: Option<Waveform> = None;
    while offset < bytes.len() {
        let header = bytes
            .get(offset..offset + 40)
            .ok_or_else(|| "MiniSEED 3 固定头被截断".to_string())?;
        if &header[..2] != b"MS" || header[2] != 3 {
            return Err(format!("偏移 {} 处不是 MiniSEED 3 记录", offset));
        }
        let sid_length = header[33] as usize;
        let extra_length = le_u16(header, 34)? as usize;
        let data_length = le_u32(header, 36)? as usize;
        let record_length = 40usize
            .checked_add(sid_length)
            .and_then(|value| value.checked_add(extra_length))
            .and_then(|value| value.checked_add(data_length))
            .ok_or_else(|| "MiniSEED 3 记录长度溢出".to_string())?;
        let record = bytes
            .get(offset..offset + record_length)
            .ok_or_else(|| "MiniSEED 3 记录长度超过文件边界".to_string())?;
        let expected_crc = le_u32(header, 28)?;
        let actual_crc = crc32c_record(record);
        if expected_crc != actual_crc {
            return Err(format!(
                "MiniSEED 3 CRC32C 不匹配: 头部 {:08x}, 计算 {:08x}",
                expected_crc, actual_crc
            ));
        }
        let sid = std::str::from_utf8(&record[40..40 + sid_length])
            .map_err(|_| "MiniSEED 3 SID 不是 UTF-8".to_string())?
            .to_string();
        let extra_start = 40 + sid_length;
        let extra_headers = std::str::from_utf8(&record[extra_start..extra_start + extra_length])
            .map_err(|_| "MiniSEED 3 extra headers 不是 UTF-8 JSON".to_string())?
            .to_string();
        if !extra_headers.is_empty()
            && !(extra_headers.trim_start().starts_with('{')
                && extra_headers.trim_end().ends_with('}'))
        {
            return Err("MiniSEED 3 extra headers 必须是 JSON 对象".to_string());
        }
        let sample_count = le_u32(header, 24)? as usize;
        let encoding = header[15];
        if matches!(encoding, 2 | 12..=18 | 30..=33) {
            return Err(format!("MiniSEED 3 禁止使用已退役编码 {}", encoding));
        }
        let rate_or_period = f64::from_bits(le_u64(header, 16)?);
        let sampling_rate_hz = if rate_or_period > 0.0 {
            rate_or_period
        } else if rate_or_period < 0.0 {
            -1.0 / rate_or_period
        } else if sample_count == 0 {
            0.0
        } else {
            return Err("MiniSEED 3 非空记录的采样率不能为 0".to_string());
        };
        if sample_count == 0 {
            offset += record_length;
            continue;
        }
        if matches!(encoding, 0 | 100) {
            return Err(format!("MiniSEED 3 编码 {} 不是数值波形", encoding));
        }
        let payload = &record[extra_start + extra_length..];
        let order = if matches!(encoding, 10 | 11 | 19) {
            ByteOrder::Big
        } else {
            ByteOrder::Little
        };
        let samples = decode_samples(payload, encoding, order, sample_count)?;
        let (network, station, location, channel) = split_fdsn_sid(&sid);
        let nanoseconds = le_u32(header, 4)?;
        let start_time = format!(
            "{:04}-{:03}T{:02}:{:02}:{:02}.{:09}Z",
            le_u16(header, 8)?,
            le_u16(header, 10)?,
            header[12],
            header[13],
            header[14],
            nanoseconds
        );
        if let Some(waveform) = &mut result {
            if waveform.source_identifier != sid {
                return Err("一个 read_waveform 调用只能读取单一 MiniSEED 3 SID".to_string());
            }
            if (waveform.sampling_rate_hz - sampling_rate_hz).abs() > 1e-9 {
                return Err("MiniSEED 3 记录采样率不一致".to_string());
            }
            waveform.samples.extend(samples);
        } else {
            result = Some(Waveform {
                samples,
                sampling_rate_hz,
                format: "MiniSEED 3".to_string(),
                source_identifier: sid,
                extra_headers,
                network,
                station,
                location,
                channel,
                start_time,
            });
        }
        offset += record_length;
    }
    result.ok_or_else(|| "MiniSEED 3 文件没有数值波形记录".to_string())
}

struct RecordHeader {
    record_length: usize,
    data_offset: usize,
    sample_count: usize,
    sampling_rate_hz: f64,
    encoding: u8,
    byte_order: ByteOrder,
    network: String,
    station: String,
    location: String,
    channel: String,
    start_time: String,
}

fn parse_record(record: &[u8]) -> Result<RecordHeader, String> {
    if record.len() < 64 || !record[..6].iter().all(u8::is_ascii_digit) {
        return Err("无效的 MiniSEED 2 固定头".to_string());
    }
    let data_offset = be_u16(record, 44)? as usize;
    let mut blockette_offset = be_u16(record, 46)? as usize;
    let mut blockette_1000 = None;
    for _ in 0..64 {
        if blockette_offset == 0 {
            break;
        }
        if blockette_offset + 8 > record.len() {
            return Err("MiniSEED blockette 越界".to_string());
        }
        let kind = be_u16(record, blockette_offset)?;
        let next = be_u16(record, blockette_offset + 2)? as usize;
        if kind == 1000 {
            blockette_1000 = Some((
                record[blockette_offset + 4],
                record[blockette_offset + 5],
                record[blockette_offset + 6],
            ));
            break;
        }
        if next == blockette_offset {
            return Err("MiniSEED blockette 链形成自环".to_string());
        }
        blockette_offset = next;
    }
    let (encoding, word_order, exponent) =
        blockette_1000.ok_or_else(|| "MiniSEED 记录缺少 blockette 1000".to_string())?;
    if !(8..=24).contains(&exponent) {
        return Err(format!("无效的 MiniSEED 记录长度指数 {}", exponent));
    }
    let record_length = 1usize << exponent;
    if data_offset < 48 || data_offset >= record_length {
        return Err("无效的 MiniSEED 数据偏移".to_string());
    }
    let factor = be_i16(record, 32)?;
    let multiplier = be_i16(record, 34)?;
    let sampling_rate_hz = seed_sample_rate(factor, multiplier)?;
    let year = be_u16(record, 20)?;
    let day = be_u16(record, 22)?;
    let fraction = be_u16(record, 28)?;
    Ok(RecordHeader {
        record_length,
        data_offset,
        sample_count: be_u16(record, 30)? as usize,
        sampling_rate_hz,
        encoding,
        byte_order: match word_order {
            0 => ByteOrder::Little,
            1 => ByteOrder::Big,
            other => return Err(format!("无效的 MiniSEED 字节序 {}", other)),
        },
        station: text_field(&record[8..13]),
        location: text_field(&record[13..15]),
        channel: text_field(&record[15..18]),
        network: text_field(&record[18..20]),
        start_time: format!(
            "{:04}-{:03}T{:02}:{:02}:{:02}.{:04}Z",
            year, day, record[24], record[25], record[26], fraction
        ),
    })
}

fn seed_sample_rate(factor: i16, multiplier: i16) -> Result<f64, String> {
    if factor == 0 || multiplier == 0 {
        return Err("MiniSEED 采样率因子不能为 0".to_string());
    }
    let rate = match (factor > 0, multiplier > 0) {
        (true, true) => factor as f64 * multiplier as f64,
        (true, false) => -(factor as f64) / multiplier as f64,
        (false, true) => -(multiplier as f64) / factor as f64,
        (false, false) => 1.0 / (factor as f64 * multiplier as f64),
    };
    Ok(rate)
}

fn decode_samples(
    data: &[u8],
    encoding: u8,
    order: ByteOrder,
    count: usize,
) -> Result<Vec<f64>, String> {
    let width = match encoding {
        1 => 2,
        2 | 12 => 3,
        3 | 4 => 4,
        5 => 8,
        13 | 14 | 16 | 30 | 32 => 2,
        10 => return decode_steim(data, order, count, false),
        11 => return decode_steim(data, order, count, true),
        19 => return Err("MiniSEED Steim-3 编码尚无稳定公开互操作实现".to_string()),
        _ => return Err(format!("不支持的 MiniSEED 编码 {}", encoding)),
    };
    if data.len() < count.saturating_mul(width) {
        return Err("MiniSEED 样本数据被截断".to_string());
    }
    let mut samples = Vec::with_capacity(count);
    for chunk in data[..count * width].chunks_exact(width) {
        let value = match encoding {
            1 => read_i16(chunk, order) as f64,
            2 | 12 => read_i24(chunk, order) as f64,
            3 => read_i32(chunk, order) as f64,
            4 => f32::from_bits(read_u32(chunk, order)) as f64,
            5 => f64::from_bits(read_u64(chunk, order)),
            13 => decode_geoscope(read_u16(chunk, order), 3),
            14 => decode_geoscope(read_u16(chunk, order), 4),
            16 => decode_cdsn(read_u16(chunk, order)) as f64,
            30 => decode_sro(read_u16(chunk, order))? as f64,
            32 => read_i16(chunk, order) as f64,
            _ => unreachable!(),
        };
        if !value.is_finite() {
            return Err("MiniSEED 包含非有限浮点样本".to_string());
        }
        samples.push(value);
    }
    Ok(samples)
}

fn decode_geoscope(sample: u16, exponent_bits: usize) -> f64 {
    let mantissa = (sample & 0x0fff) as i32 - 2048;
    let exponent_mask = if exponent_bits == 3 { 0x7000 } else { 0xf000 };
    let exponent = ((sample & exponent_mask) >> 12) as i32;
    mantissa as f64 / 2.0_f64.powi(exponent)
}

fn decode_cdsn(sample: u16) -> i32 {
    let mantissa = (sample & 0x3fff) as i32 - 8191;
    let shift = [0, 2, 4, 7][(sample >> 14) as usize];
    mantissa * (1 << shift)
}

fn decode_sro(sample: u16) -> Result<i32, String> {
    let mantissa = sign_extend((sample & 0x0fff) as u32, 12);
    let exponent = 10 - ((sample >> 12) as i32);
    if !(0..=10).contains(&exponent) {
        return Err("SRO gain range 指数超出 0..10".to_string());
    }
    Ok(mantissa * (1 << exponent))
}

fn decode_steim(
    data: &[u8],
    order: ByteOrder,
    count: usize,
    steim2: bool,
) -> Result<Vec<f64>, String> {
    if data.len() < 64 || data.len() % 64 != 0 {
        return Err("STEIM 数据区必须由完整的 64 字节帧组成".to_string());
    }
    let first = &data[..64];
    let x0 = read_i32(&first[4..8], order);
    let xn = read_i32(&first[8..12], order);
    let mut current = x0;
    let mut samples = vec![x0 as f64];
    'frames: for (frame_index, frame) in data.chunks_exact(64).enumerate() {
        let control = read_u32(&frame[..4], order);
        for word_index in 1..16 {
            if frame_index == 0 && word_index <= 2 {
                continue;
            }
            let code = (control >> (30 - 2 * word_index)) & 0b11;
            let word = read_u32(&frame[word_index * 4..word_index * 4 + 4], order);
            let differences = if steim2 {
                steim2_differences(code, word)?
            } else {
                steim1_differences(code, word)
            };
            for difference in differences {
                current = current.wrapping_add(difference);
                samples.push(current as f64);
                if samples.len() == count {
                    break 'frames;
                }
            }
        }
    }
    if samples.len() != count {
        return Err(format!(
            "STEIM 解压得到 {} 个样本，头部声明 {} 个",
            samples.len(),
            count
        ));
    }
    if count > 0 && current != xn {
        return Err(format!("STEIM 反向积分常量不匹配: {} != {}", current, xn));
    }
    Ok(samples)
}

fn steim1_differences(code: u32, word: u32) -> Vec<i32> {
    match code {
        0 => vec![],
        1 => unpack(word, 4, 8, 32),
        2 => unpack(word, 2, 16, 32),
        3 => vec![word as i32],
        _ => unreachable!(),
    }
}

fn steim2_differences(code: u32, word: u32) -> Result<Vec<i32>, String> {
    let dnib = word >> 30;
    match (code, dnib) {
        (0, _) => Ok(vec![]),
        (1, _) => Ok(unpack(word, 4, 8, 32)),
        (2, 1) => Ok(unpack(word & 0x3fff_ffff, 1, 30, 30)),
        (2, 2) => Ok(unpack(word & 0x3fff_ffff, 2, 15, 30)),
        (2, 3) => Ok(unpack(word & 0x3fff_ffff, 3, 10, 30)),
        (3, 0) => Ok(unpack(word & 0x3fff_ffff, 5, 6, 30)),
        (3, 1) => Ok(unpack(word & 0x3fff_ffff, 6, 5, 30)),
        (3, 2) => Ok(unpack(word & 0x3fff_ffff, 7, 4, 28)),
        _ => Err("无效的 STEIM-2 控制码".to_string()),
    }
}

fn unpack(word: u32, count: usize, width: usize, used_bits: usize) -> Vec<i32> {
    let mask = if width == 32 {
        u32::MAX
    } else {
        (1u32 << width) - 1
    };
    (0..count)
        .map(|index| {
            let shift = used_bits - width * (index + 1);
            sign_extend((word >> shift) & mask, width)
        })
        .collect()
}

fn sign_extend(value: u32, bits: usize) -> i32 {
    if bits == 32 {
        value as i32
    } else {
        ((value << (32 - bits)) as i32) >> (32 - bits)
    }
}

/// 四极点 Butterworth 带通，双向执行获得零相位响应。
pub fn bandpass_zero_phase(
    samples: &[f64],
    sampling_rate_hz: f64,
    low_hz: f64,
    high_hz: f64,
) -> Result<Vec<f64>, String> {
    if samples.len() < 8 || samples.iter().any(|value| !value.is_finite()) {
        return Err("bandpass 至少需要 8 个有限样本".to_string());
    }
    if !sampling_rate_hz.is_finite()
        || !low_hz.is_finite()
        || !high_hz.is_finite()
        || sampling_rate_hz <= 0.0
        || low_hz <= 0.0
        || high_hz <= low_hz
        || high_hz >= sampling_rate_hz / 2.0
    {
        return Err("bandpass 要求 0 < low_hz < high_hz < Nyquist".to_string());
    }
    let q = 1.0 / 2.0_f64.sqrt();
    let high_pass = design_biquad(sampling_rate_hz, low_hz, q, true);
    let low_pass = design_biquad(sampling_rate_hz, high_hz, q, false);
    let pad = (samples.len() - 1).min(12);
    let mut filtered = Vec::with_capacity(samples.len() + 2 * pad);
    filtered.extend(
        (1..=pad)
            .rev()
            .map(|index| 2.0 * samples[0] - samples[index]),
    );
    filtered.extend_from_slice(samples);
    filtered.extend(
        (1..=pad)
            .map(|index| 2.0 * samples[samples.len() - 1] - samples[samples.len() - 1 - index]),
    );
    high_pass.apply(&mut filtered);
    low_pass.apply(&mut filtered);
    filtered.reverse();
    high_pass.apply(&mut filtered);
    low_pass.apply(&mut filtered);
    filtered.reverse();
    Ok(filtered[pad..pad + samples.len()].to_vec())
}

fn design_biquad(rate: f64, cutoff: f64, q: f64, high_pass: bool) -> Biquad {
    let omega = 2.0 * PI * cutoff / rate;
    let (sin, cos) = omega.sin_cos();
    let alpha = sin / (2.0 * q);
    let a0 = 1.0 + alpha;
    let (b0, b1, b2) = if high_pass {
        ((1.0 + cos) / 2.0, -(1.0 + cos), (1.0 + cos) / 2.0)
    } else {
        ((1.0 - cos) / 2.0, 1.0 - cos, (1.0 - cos) / 2.0)
    };
    Biquad {
        b0: b0 / a0,
        b1: b1 / a0,
        b2: b2 / a0,
        a1: -2.0 * cos / a0,
        a2: (1.0 - alpha) / a0,
    }
}

/// 使用阻尼最小二乘求六分量全矩张量。
pub fn invert_moment_tensor(
    greens: &[Vec<f64>],
    observations: &[f64],
    damping: f64,
) -> Result<InversionResult, String> {
    if greens.len() != observations.len() || greens.len() < 6 {
        return Err("反演要求观测数与 Green 矩阵行数一致，且至少 6 行".to_string());
    }
    if damping < 0.0
        || !damping.is_finite()
        || observations.iter().any(|v| !v.is_finite())
        || greens
            .iter()
            .any(|row| row.len() != 6 || row.iter().any(|v| !v.is_finite()))
    {
        return Err("反演输入必须是有限值、6 列 Green 矩阵和非负阻尼".to_string());
    }
    let mut normal = vec![vec![0.0; 6]; 6];
    let mut rhs = vec![0.0; 6];
    for (row, observed) in greens.iter().zip(observations) {
        for i in 0..6 {
            rhs[i] += row[i] * observed;
            for j in 0..6 {
                normal[i][j] += row[i] * row[j];
            }
        }
    }
    for (index, row) in normal.iter_mut().enumerate() {
        row[index] += damping * damping;
    }
    let (moment_tensor, condition_proxy) = solve(normal, rhs)?;
    let predicted = greens
        .iter()
        .map(|row| row.iter().zip(&moment_tensor).map(|(a, b)| a * b).sum())
        .collect::<Vec<f64>>();
    let residuals = observations
        .iter()
        .zip(&predicted)
        .map(|(a, b)| a - b)
        .collect::<Vec<_>>();
    let residual_energy: f64 = residuals.iter().map(|v| v * v).sum();
    let data_energy: f64 = observations.iter().map(|v| v * v).sum();
    let rms = (residual_energy / observations.len() as f64).sqrt();
    let variance_reduction = if data_energy > f64::EPSILON {
        1.0 - residual_energy / data_energy
    } else if residual_energy <= f64::EPSILON {
        1.0
    } else {
        0.0
    };
    Ok(InversionResult {
        moment_tensor,
        predicted,
        residuals,
        rms,
        variance_reduction,
        condition_proxy,
    })
}

/// 均匀各向同性全空间中的远场 P/S 波幅格林矩阵。
pub fn homogeneous_greens(
    stations_km: &[[f64; 3]],
    source_km: [f64; 3],
    medium: ElasticMedium,
    phase: &str,
) -> Result<GreensResult, String> {
    if stations_km.is_empty()
        || !matches!(phase, "P" | "S")
        || medium.vp_km_s <= medium.vs_km_s
        || medium.vs_km_s <= 0.0
        || medium.density_kg_m3 <= 0.0
        || [medium.vp_km_s, medium.vs_km_s, medium.density_kg_m3]
            .iter()
            .any(|value| !value.is_finite())
    {
        return Err("green_functions 要求有效台站、P/S 相、vp>vs>0 和 density>0".to_string());
    }
    let velocity_km_s = if phase == "P" {
        medium.vp_km_s
    } else {
        medium.vs_km_s
    };
    let velocity_m_s = velocity_km_s * 1000.0;
    let mut matrix = Vec::with_capacity(stations_km.len() * 3);
    let mut travel_times_s = Vec::with_capacity(stations_km.len());
    for station in stations_km {
        if station
            .iter()
            .chain(source_km.iter())
            .any(|value| !value.is_finite())
        {
            return Err("台站和震源坐标必须是有限值".to_string());
        }
        let delta = [
            station[0] - source_km[0],
            station[1] - source_km[1],
            station[2] - source_km[2],
        ];
        let distance_km = (delta.iter().map(|value| value * value).sum::<f64>()).sqrt();
        if distance_km <= 0.0 {
            return Err("台站不能与震源位于同一点".to_string());
        }
        let n = [
            delta[0] / distance_km,
            delta[1] / distance_km,
            delta[2] / distance_km,
        ];
        let q = [
            n[0] * n[0],
            n[1] * n[1],
            n[2] * n[2],
            2.0 * n[0] * n[1],
            2.0 * n[0] * n[2],
            2.0 * n[1] * n[2],
        ];
        let factor =
            1.0 / (4.0 * PI * medium.density_kg_m3 * velocity_m_s.powi(3) * distance_km * 1000.0);
        for component in 0..3 {
            let mn = match component {
                0 => [n[0], 0.0, 0.0, n[1], n[2], 0.0],
                1 => [0.0, n[1], 0.0, n[0], 0.0, n[2]],
                _ => [0.0, 0.0, n[2], 0.0, n[0], n[1]],
            };
            matrix.push(
                (0..6)
                    .map(|index| {
                        if phase == "P" {
                            factor * n[component] * q[index]
                        } else {
                            factor * (mn[index] - n[component] * q[index])
                        }
                    })
                    .collect(),
            );
        }
        travel_times_s.push(distance_km / velocity_km_s);
    }
    Ok(GreensResult {
        matrix,
        travel_times_s,
    })
}

/// 矩形网格有限断层反演：非负分布矩 + 平滑/阻尼 + 几何角度坐标搜索。
pub fn invert_finite_fault(
    stations_km: &[[f64; 3]],
    observations: &[f64],
    source_km: [f64; 3],
    initial: FaultGeometry,
    medium: ElasticMedium,
    phase: &str,
    damping: f64,
    smoothing: f64,
    inner_iterations: usize,
) -> Result<FiniteFaultResult, String> {
    validate_fault(
        initial,
        stations_km,
        observations,
        damping,
        smoothing,
        inner_iterations,
    )?;
    let mut evaluations = 1;
    let mut best = evaluate_fault(
        stations_km,
        observations,
        source_km,
        initial,
        medium,
        phase,
        damping,
        smoothing,
        inner_iterations,
    )?;
    for step in [20.0, 10.0, 5.0, 2.0, 1.0] {
        for parameter in 0..3 {
            for direction in [-1.0, 1.0] {
                let mut candidate = best.geometry;
                match parameter {
                    0 => {
                        candidate.strike_deg =
                            wrap(candidate.strike_deg + direction * step, 0.0, 360.0)
                    }
                    1 => {
                        candidate.dip_deg = (candidate.dip_deg + direction * step).clamp(1.0, 89.0)
                    }
                    _ => {
                        candidate.rake_deg =
                            wrap(candidate.rake_deg + direction * step, -180.0, 180.0)
                    }
                }
                let result = evaluate_fault(
                    stations_km,
                    observations,
                    source_km,
                    candidate,
                    medium,
                    phase,
                    damping,
                    smoothing,
                    inner_iterations,
                )?;
                evaluations += 1;
                if result.objective < best.objective {
                    best = result;
                }
            }
        }
    }
    best.model_evaluations = evaluations;
    Ok(best)
}

fn validate_fault(
    fault: FaultGeometry,
    stations: &[[f64; 3]],
    observations: &[f64],
    damping: f64,
    smoothing: f64,
    iterations: usize,
) -> Result<(), String> {
    let patch_count = fault.nstrike.saturating_mul(fault.ndip);
    if observations.len() != stations.len() * 3
        || stations.is_empty()
        || fault.length_km <= 0.0
        || fault.width_km <= 0.0
        || !(1.0..90.0).contains(&fault.dip_deg)
        || patch_count == 0
        || patch_count > 1024
        || iterations == 0
        || damping < 0.0
        || smoothing < 0.0
        || observations.iter().any(|value| !value.is_finite())
    {
        return Err("finite_fault_inversion 输入、几何或网格无效".to_string());
    }
    Ok(())
}

fn evaluate_fault(
    stations: &[[f64; 3]],
    observations: &[f64],
    source: [f64; 3],
    geometry: FaultGeometry,
    medium: ElasticMedium,
    phase: &str,
    damping: f64,
    smoothing: f64,
    iterations: usize,
) -> Result<FiniteFaultResult, String> {
    let (design, centers, patch_area_m2) = fault_design(stations, source, geometry, medium, phase)?;
    let (moments, objective) = projected_least_squares(
        &design,
        observations,
        geometry.nstrike,
        geometry.ndip,
        damping,
        smoothing,
        iterations,
    )?;
    let predicted = (0..observations.len())
        .map(|row| design[row].iter().zip(&moments).map(|(a, b)| a * b).sum())
        .collect::<Vec<f64>>();
    let residuals = observations
        .iter()
        .zip(&predicted)
        .map(|(observed, predicted)| observed - predicted)
        .collect::<Vec<_>>();
    let residual_energy: f64 = residuals.iter().map(|value| value * value).sum();
    let data_energy: f64 = observations.iter().map(|value| value * value).sum();
    let rms = (residual_energy / observations.len() as f64).sqrt();
    let variance_reduction = if data_energy > f64::EPSILON {
        1.0 - residual_energy / data_energy
    } else {
        0.0
    };
    let shear_modulus = medium.density_kg_m3 * (medium.vs_km_s * 1000.0).powi(2);
    let patch_slips_m = moments
        .iter()
        .map(|moment| moment / (shear_modulus * patch_area_m2))
        .collect();
    Ok(FiniteFaultResult {
        geometry,
        patch_centers_km: centers,
        patch_moments_nm: moments,
        patch_slips_m,
        predicted,
        residuals,
        rms,
        variance_reduction,
        objective,
        model_evaluations: 1,
    })
}

fn fault_design(
    stations: &[[f64; 3]],
    source: [f64; 3],
    fault: FaultGeometry,
    medium: ElasticMedium,
    phase: &str,
) -> Result<(Vec<Vec<f64>>, Vec<[f64; 3]>, f64), String> {
    let strike = fault.strike_deg.to_radians();
    let dip = fault.dip_deg.to_radians();
    let rake = fault.rake_deg.to_radians();
    let strike_vector = [strike.sin(), strike.cos(), 0.0];
    let dip_vector = [
        strike.cos() * dip.cos(),
        -strike.sin() * dip.cos(),
        dip.sin(),
    ];
    let normal = [
        -strike.cos() * dip.sin(),
        strike.sin() * dip.sin(),
        dip.cos(),
    ];
    let slip = (0..3)
        .map(|index| rake.cos() * strike_vector[index] + rake.sin() * dip_vector[index])
        .collect::<Vec<_>>();
    let tensor = [
        2.0 * slip[0] * normal[0],
        2.0 * slip[1] * normal[1],
        2.0 * slip[2] * normal[2],
        slip[0] * normal[1] + normal[0] * slip[1],
        slip[0] * normal[2] + normal[0] * slip[2],
        slip[1] * normal[2] + normal[1] * slip[2],
    ];
    let mut centers = Vec::with_capacity(fault.nstrike * fault.ndip);
    let ds = fault.length_km / fault.nstrike as f64;
    let dd = fault.width_km / fault.ndip as f64;
    for dip_index in 0..fault.ndip {
        for strike_index in 0..fault.nstrike {
            let along = (strike_index as f64 + 0.5 - fault.nstrike as f64 / 2.0) * ds;
            let down = (dip_index as f64 + 0.5 - fault.ndip as f64 / 2.0) * dd;
            centers.push([
                source[0] + along * strike_vector[0] + down * dip_vector[0],
                source[1] + along * strike_vector[1] + down * dip_vector[1],
                source[2] + along * strike_vector[2] + down * dip_vector[2],
            ]);
        }
    }
    let mut design = vec![vec![0.0; centers.len()]; stations.len() * 3];
    for (column, center) in centers.iter().enumerate() {
        let greens = homogeneous_greens(stations, *center, medium, phase)?;
        for (row, coefficients) in greens.matrix.iter().enumerate() {
            design[row][column] = coefficients.iter().zip(tensor).map(|(a, b)| a * b).sum();
        }
    }
    Ok((design, centers, ds * dd * 1_000_000.0))
}

fn projected_least_squares(
    matrix: &[Vec<f64>],
    data: &[f64],
    nstrike: usize,
    ndip: usize,
    damping: f64,
    smoothing: f64,
    iterations: usize,
) -> Result<(Vec<f64>, f64), String> {
    let columns = nstrike * ndip;
    let mut normal = vec![vec![0.0; columns]; columns];
    let mut rhs = vec![0.0; columns];
    for (row, observed) in matrix.iter().zip(data) {
        for i in 0..columns {
            rhs[i] += row[i] * observed;
            for j in 0..columns {
                normal[i][j] += row[i] * row[j];
            }
        }
    }
    for index in 0..columns {
        normal[index][index] += damping * damping;
    }
    for dip in 0..ndip {
        for strike in 0..nstrike {
            let index = dip * nstrike + strike;
            for neighbor in [
                (strike + 1 < nstrike).then_some(index + 1),
                (dip + 1 < ndip).then_some(index + nstrike),
            ]
            .into_iter()
            .flatten()
            {
                let weight = smoothing * smoothing;
                normal[index][index] += weight;
                normal[neighbor][neighbor] += weight;
                normal[index][neighbor] -= weight;
                normal[neighbor][index] -= weight;
            }
        }
    }
    let lipschitz = normal
        .iter()
        .map(|row| row.iter().map(|value| value.abs()).sum::<f64>())
        .fold(0.0, f64::max);
    if lipschitz <= f64::EPSILON {
        return Err("有限断层设计矩阵没有可辨识能量".to_string());
    }
    let mut model = vec![0.0; columns];
    for _ in 0..iterations {
        let gradient = (0..columns)
            .map(|row| {
                normal[row]
                    .iter()
                    .zip(&model)
                    .map(|(a, b)| a * b)
                    .sum::<f64>()
                    - rhs[row]
            })
            .collect::<Vec<_>>();
        let next = model
            .iter()
            .zip(gradient)
            .map(|(value, gradient)| (value - gradient / lipschitz).max(0.0))
            .collect::<Vec<_>>();
        let change = next
            .iter()
            .zip(&model)
            .map(|(a, b)| (a - b).abs())
            .fold(0.0, f64::max);
        model = next;
        if change <= 1e-10 * (1.0 + model.iter().copied().fold(0.0, f64::max)) {
            break;
        }
    }
    let residual_energy: f64 = matrix
        .iter()
        .zip(data)
        .map(|(row, observed)| {
            let predicted: f64 = row.iter().zip(&model).map(|(a, b)| a * b).sum();
            (observed - predicted).powi(2)
        })
        .sum();
    let objective = residual_energy
        + damping.powi(2) * model.iter().map(|v| v * v).sum::<f64>()
        + smoothing.powi(2) * neighbor_energy(&model, nstrike, ndip);
    Ok((model, objective))
}

fn neighbor_energy(model: &[f64], nstrike: usize, ndip: usize) -> f64 {
    let mut energy = 0.0;
    for dip in 0..ndip {
        for strike in 0..nstrike {
            let index = dip * nstrike + strike;
            if strike + 1 < nstrike {
                energy += (model[index] - model[index + 1]).powi(2);
            }
            if dip + 1 < ndip {
                energy += (model[index] - model[index + nstrike]).powi(2);
            }
        }
    }
    energy
}

fn wrap(value: f64, minimum: f64, maximum: f64) -> f64 {
    (value - minimum).rem_euclid(maximum - minimum) + minimum
}

fn solve(mut matrix: Vec<Vec<f64>>, mut rhs: Vec<f64>) -> Result<(Vec<f64>, f64), String> {
    let n = rhs.len();
    let mut max_pivot: f64 = 0.0;
    let mut min_pivot = f64::INFINITY;
    for column in 0..n {
        let pivot_row = (column..n)
            .max_by(|a, b| {
                matrix[*a][column]
                    .abs()
                    .total_cmp(&matrix[*b][column].abs())
            })
            .unwrap();
        matrix.swap(column, pivot_row);
        rhs.swap(column, pivot_row);
        let pivot = matrix[column][column].abs();
        if pivot <= 1e-12 {
            return Err("Green 矩阵秩不足；请增加独立观测或设置阻尼".to_string());
        }
        max_pivot = max_pivot.max(pivot);
        min_pivot = min_pivot.min(pivot);
        for row in column + 1..n {
            let factor = matrix[row][column] / matrix[column][column];
            for col in column..n {
                matrix[row][col] -= factor * matrix[column][col];
            }
            rhs[row] -= factor * rhs[column];
        }
    }
    let mut solution = vec![0.0; n];
    for row in (0..n).rev() {
        let tail: f64 = (row + 1..n)
            .map(|col| matrix[row][col] * solution[col])
            .sum();
        solution[row] = (rhs[row] - tail) / matrix[row][row];
    }
    Ok((solution, max_pivot / min_pivot))
}

fn text_field(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes)
        .trim_matches(|c| c == ' ' || c == '\0')
        .to_string()
}

fn be_u16(bytes: &[u8], offset: usize) -> Result<u16, String> {
    bytes
        .get(offset..offset + 2)
        .and_then(|v| v.try_into().ok())
        .map(u16::from_be_bytes)
        .ok_or_else(|| "MiniSEED 头部字段越界".to_string())
}

fn be_i16(bytes: &[u8], offset: usize) -> Result<i16, String> {
    be_u16(bytes, offset).map(|value| value as i16)
}

fn read_u32(bytes: &[u8], order: ByteOrder) -> u32 {
    let value: [u8; 4] = bytes.try_into().unwrap();
    match order {
        ByteOrder::Big => u32::from_be_bytes(value),
        ByteOrder::Little => u32::from_le_bytes(value),
    }
}

fn read_u16(bytes: &[u8], order: ByteOrder) -> u16 {
    let value: [u8; 2] = bytes.try_into().unwrap();
    match order {
        ByteOrder::Big => u16::from_be_bytes(value),
        ByteOrder::Little => u16::from_le_bytes(value),
    }
}

fn read_i16(bytes: &[u8], order: ByteOrder) -> i16 {
    let value: [u8; 2] = bytes.try_into().unwrap();
    match order {
        ByteOrder::Big => i16::from_be_bytes(value),
        ByteOrder::Little => i16::from_le_bytes(value),
    }
}

fn read_i24(bytes: &[u8], order: ByteOrder) -> i32 {
    let raw = match order {
        ByteOrder::Big => ((bytes[0] as u32) << 16) | ((bytes[1] as u32) << 8) | bytes[2] as u32,
        ByteOrder::Little => ((bytes[2] as u32) << 16) | ((bytes[1] as u32) << 8) | bytes[0] as u32,
    };
    sign_extend(raw, 24)
}

fn read_i32(bytes: &[u8], order: ByteOrder) -> i32 {
    read_u32(bytes, order) as i32
}

fn read_u64(bytes: &[u8], order: ByteOrder) -> u64 {
    let value: [u8; 8] = bytes.try_into().unwrap();
    match order {
        ByteOrder::Big => u64::from_be_bytes(value),
        ByteOrder::Little => u64::from_le_bytes(value),
    }
}

fn le_u16(bytes: &[u8], offset: usize) -> Result<u16, String> {
    bytes
        .get(offset..offset + 2)
        .and_then(|value| value.try_into().ok())
        .map(u16::from_le_bytes)
        .ok_or_else(|| "MiniSEED 3 字段越界".to_string())
}

fn le_u32(bytes: &[u8], offset: usize) -> Result<u32, String> {
    bytes
        .get(offset..offset + 4)
        .and_then(|value| value.try_into().ok())
        .map(u32::from_le_bytes)
        .ok_or_else(|| "MiniSEED 3 字段越界".to_string())
}

fn le_u64(bytes: &[u8], offset: usize) -> Result<u64, String> {
    bytes
        .get(offset..offset + 8)
        .and_then(|value| value.try_into().ok())
        .map(u64::from_le_bytes)
        .ok_or_else(|| "MiniSEED 3 字段越界".to_string())
}

fn crc32c_record(record: &[u8]) -> u32 {
    let mut crc = !0u32;
    for (index, byte) in record.iter().enumerate() {
        crc ^= if (28..32).contains(&index) { 0 } else { *byte } as u32;
        for _ in 0..8 {
            crc = (crc >> 1) ^ (0x82f63b78 & (0u32.wrapping_sub(crc & 1)));
        }
    }
    !crc
}

fn split_fdsn_sid(sid: &str) -> (String, String, String, String) {
    let parts = sid
        .strip_prefix("FDSN:")
        .unwrap_or(sid)
        .split('_')
        .collect::<Vec<_>>();
    if parts.len() >= 6 {
        (
            parts[0].to_string(),
            parts[1].to_string(),
            parts[2].to_string(),
            format!("{}{}{}", parts[3], parts[4], parts[5]),
        )
    } else {
        (String::new(), String::new(), String::new(), String::new())
    }
}
