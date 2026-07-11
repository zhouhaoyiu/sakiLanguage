use saki_lang::seismic::{
    ElasticMedium, bandpass_zero_phase, homogeneous_greens, invert_moment_tensor, parse_miniseed,
};
use std::f64::consts::PI;

#[test]
fn parses_big_endian_int32_miniseed_record() {
    let mut record = vec![0u8; 256];
    record[..8].copy_from_slice(b"000001D ");
    record[8..13].copy_from_slice(b"SAKI ");
    record[13..15].copy_from_slice(b"00");
    record[15..18].copy_from_slice(b"HNZ");
    record[18..20].copy_from_slice(b"SK");
    record[20..22].copy_from_slice(&2026u16.to_be_bytes());
    record[22..24].copy_from_slice(&1u16.to_be_bytes());
    record[30..32].copy_from_slice(&4u16.to_be_bytes());
    record[32..34].copy_from_slice(&100i16.to_be_bytes());
    record[34..36].copy_from_slice(&1i16.to_be_bytes());
    record[39] = 1;
    record[44..46].copy_from_slice(&64u16.to_be_bytes());
    record[46..48].copy_from_slice(&48u16.to_be_bytes());
    record[48..50].copy_from_slice(&1000u16.to_be_bytes());
    record[52] = 3;
    record[53] = 1;
    record[54] = 8;
    for (index, sample) in [-2i32, 0, 7, 42].iter().enumerate() {
        record[64 + index * 4..68 + index * 4].copy_from_slice(&sample.to_be_bytes());
    }

    let waveform = parse_miniseed(&record).unwrap();
    assert_eq!(waveform.samples, vec![-2.0, 0.0, 7.0, 42.0]);
    assert_eq!(waveform.sampling_rate_hz, 100.0);
    assert_eq!(waveform.station, "SAKI");
}

#[test]
fn zero_phase_bandpass_preserves_passband_more_than_stopband() {
    let rate = 100.0;
    let samples = (0..1000)
        .map(|index| {
            let t = index as f64 / rate;
            (2.0 * PI * 5.0 * t).sin() + (2.0 * PI * 30.0 * t).sin()
        })
        .collect::<Vec<_>>();
    let filtered = bandpass_zero_phase(&samples, rate, 2.0, 10.0).unwrap();
    let amplitude = |frequency: f64| {
        filtered[100..900]
            .iter()
            .enumerate()
            .map(|(index, value)| {
                value * (2.0 * PI * frequency * (index + 100) as f64 / rate).sin()
            })
            .sum::<f64>()
            .abs()
    };
    assert!(amplitude(5.0) > 20.0 * amplitude(30.0));
}

#[test]
fn full_moment_tensor_inversion_recovers_known_solution() {
    let greens = (0..6)
        .map(|row| {
            (0..6)
                .map(|column| if row == column { 1.0 } else { 0.0 })
                .collect()
        })
        .collect::<Vec<Vec<f64>>>();
    let expected = vec![1.0, -2.0, 3.0, 4.0, -5.0, 6.0];
    let result = invert_moment_tensor(&greens, &expected, 0.0).unwrap();
    assert_eq!(result.moment_tensor, expected);
    assert!(result.rms < 1e-12);
    assert!((result.variance_reduction - 1.0).abs() < 1e-12);
}

#[test]
fn inversion_rejects_rank_deficient_green_matrix() {
    let greens = vec![vec![1.0; 6]; 6];
    let error = invert_moment_tensor(&greens, &[1.0; 6], 0.0).unwrap_err();
    assert!(error.contains("秩不足"));
}

#[test]
fn parses_miniseed3_with_crc32c() {
    let sid = b"FDSN:SK_SAKI_00_H_N_Z";
    let samples = [-2i32, 0, 7, 42];
    let mut record = vec![0u8; 40 + sid.len() + samples.len() * 4];
    record[..3].copy_from_slice(b"MS\x03");
    record[8..10].copy_from_slice(&2026u16.to_le_bytes());
    record[10..12].copy_from_slice(&1u16.to_le_bytes());
    record[15] = 3;
    record[16..24].copy_from_slice(&100.0f64.to_le_bytes());
    record[24..28].copy_from_slice(&(samples.len() as u32).to_le_bytes());
    record[32] = 1;
    record[33] = sid.len() as u8;
    record[36..40].copy_from_slice(&((samples.len() * 4) as u32).to_le_bytes());
    record[40..40 + sid.len()].copy_from_slice(sid);
    for (index, sample) in samples.iter().enumerate() {
        let start = 40 + sid.len() + index * 4;
        record[start..start + 4].copy_from_slice(&sample.to_le_bytes());
    }
    let crc = test_crc32c(&record);
    record[28..32].copy_from_slice(&crc.to_le_bytes());
    let waveform = parse_miniseed(&record).unwrap();
    assert_eq!(waveform.format, "MiniSEED 3");
    assert_eq!(waveform.samples, vec![-2.0, 0.0, 7.0, 42.0]);
    assert_eq!(waveform.channel, "HNZ");
}

#[test]
fn homogeneous_green_matrix_has_three_rows_per_station() {
    let medium = ElasticMedium {
        vp_km_s: 6.0,
        vs_km_s: 35.0 / 10.0,
        density_kg_m3: 2700.0,
    };
    let result =
        homogeneous_greens(&[[10.0, 0.0, 0.0], [0.0, 20.0, 0.0]], [0.0; 3], medium, "P").unwrap();
    assert_eq!(result.matrix.len(), 6);
    assert!(result.matrix.iter().all(|row| row.len() == 6));
    assert_eq!(result.travel_times_s.len(), 2);
}

fn test_crc32c(record: &[u8]) -> u32 {
    let mut crc = !0u32;
    for (index, byte) in record.iter().enumerate() {
        crc ^= if (28..32).contains(&index) { 0 } else { *byte } as u32;
        for _ in 0..8 {
            crc = (crc >> 1) ^ (0x82f63b78 & (0u32.wrapping_sub(crc & 1)));
        }
    }
    !crc
}
