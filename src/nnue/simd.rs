//! AVX2 kernels for NNUE inference, with scalar fallback.

const WEIGHT_SCALE_BITS: i32 = 6;

#[inline]
fn clip_relu(x: i32) -> u8 {
    let v = x >> WEIGHT_SCALE_BITS;
    if v < 0 {
        0
    } else if v > 127 {
        127
    } else {
        v as u8
    }
}

pub fn clamp_acc(acc: &[i16; 512], out: &mut [u8]) {
    #[cfg(target_arch = "x86_64")]
    {
        if is_x86_feature_detected!("avx2") {
            unsafe {
                clamp_acc_avx2(acc, out);
            }
            return;
        }
    }
    clamp_acc_scalar(acc, out);
}

pub fn fc0_relu(weights: &[i8], input: &[u8; 1024], bias: &[i32; 16], out: &mut [u8; 16]) {
    #[cfg(target_arch = "x86_64")]
    {
        if is_x86_feature_detected!("avx2") {
            unsafe {
                fc0_relu_avx2(weights, input, bias, out);
            }
            return;
        }
    }
    fc0_relu_scalar(weights, input, bias, out);
}

pub fn add_i16x512(dst: &mut [i16; 512], src: &[i16], sign: i16) {
    debug_assert!(src.len() >= 512);
    #[cfg(target_arch = "x86_64")]
    {
        if is_x86_feature_detected!("avx2") {
            unsafe {
                add_i16x512_avx2(dst, src, sign);
            }
            return;
        }
    }
    add_i16x512_scalar(dst, src, sign);
}

pub fn fc1_relu(weights: &[i8], input: &[u8; 16], bias: &[i32; 32], out: &mut [u8; 32]) {
    #[cfg(target_arch = "x86_64")]
    {
        if is_x86_feature_detected!("avx2") {
            unsafe {
                fc1_relu_avx2(weights, input, bias, out);
            }
            return;
        }
    }
    fc1_relu_scalar(weights, input, bias, out);
}

pub fn fc2(weights: &[i8; 32], input: &[u8; 32], bias: i32) -> i32 {
    #[cfg(target_arch = "x86_64")]
    {
        if is_x86_feature_detected!("avx2") {
            return unsafe { fc2_avx2(weights, input, bias) };
        }
    }
    fc2_scalar(weights, input, bias)
}

pub(crate) fn fc0_relu_scalar(
    weights: &[i8],
    input: &[u8; 1024],
    bias: &[i32; 16],
    out: &mut [u8; 16],
) {
    for i in 0..16 {
        let mut sum = bias[i];
        let row = &weights[i * 1024..i * 1024 + 1024];
        for j in 0..1024 {
            sum += row[j] as i32 * input[j] as i32;
        }
        out[i] = clip_relu(sum);
    }
}

fn fc1_relu_scalar(
    weights: &[i8],
    input: &[u8; 16],
    bias: &[i32; 32],
    out: &mut [u8; 32],
) {
    for i in 0..32 {
        let mut sum = bias[i];
        let row = &weights[i * 32..i * 32 + 16];
        for j in 0..16 {
            sum += row[j] as i32 * input[j] as i32;
        }
        out[i] = clip_relu(sum);
    }
}

fn fc2_scalar(weights: &[i8; 32], input: &[u8; 32], bias: i32) -> i32 {
    let mut sum = bias;
    for j in 0..32 {
        sum += weights[j] as i32 * input[j] as i32;
    }
    sum
}

fn clamp_acc_scalar(acc: &[i16; 512], out: &mut [u8]) {
    for i in 0..512 {
        let v = acc[i] as i32;
        out[i] = if v < 0 {
            0
        } else if v > 127 {
            127
        } else {
            v as u8
        };
    }
}

fn add_i16x512_scalar(dst: &mut [i16; 512], src: &[i16], sign: i16) {
    for j in 0..512 {
        dst[j] = dst[j].wrapping_add(src[j].wrapping_mul(sign));
    }
}

#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "avx2")]
unsafe fn clamp_acc_avx2(acc: &[i16; 512], out: &mut [u8]) {
    use std::arch::x86_64::*;
    let zero = _mm256_setzero_si256();
    let maxv = _mm256_set1_epi16(127);
    let ap = acc.as_ptr();
    let op = out.as_mut_ptr();
    let mut i = 0;
    while i < 512 {
        let a = _mm256_loadu_si256(ap.add(i) as *const __m256i);
        let b = _mm256_loadu_si256(ap.add(i + 16) as *const __m256i);
        let a = _mm256_min_epi16(_mm256_max_epi16(a, zero), maxv);
        let b = _mm256_min_epi16(_mm256_max_epi16(b, zero), maxv);
        let packed = _mm256_packus_epi16(a, b);
        let packed = _mm256_permute4x64_epi64::<0xD8>(packed);
        _mm256_storeu_si256(op.add(i) as *mut __m256i, packed);
        i += 32;
    }
}

#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "avx2")]
unsafe fn fc0_relu_avx2(weights: &[i8], input: &[u8; 1024], bias: &[i32; 16], out: &mut [u8; 16]) {
    use std::arch::x86_64::*;
    let ones = _mm256_set1_epi16(1);
    let xp = input.as_ptr();
    let wp = weights.as_ptr();
    for r in (0..16).step_by(4) {
        let mut acc0 = _mm256_setzero_si256();
        let mut acc1 = _mm256_setzero_si256();
        let mut acc2 = _mm256_setzero_si256();
        let mut acc3 = _mm256_setzero_si256();
        let w0 = wp.add(r * 1024);
        let w1 = wp.add((r + 1) * 1024);
        let w2 = wp.add((r + 2) * 1024);
        let w3 = wp.add((r + 3) * 1024);
        let mut j = 0;
        while j < 1024 {
            let x = _mm256_loadu_si256(xp.add(j) as *const __m256i);
            acc0 = _mm256_add_epi32(
                acc0,
                _mm256_madd_epi16(_mm256_maddubs_epi16(x, _mm256_loadu_si256(w0.add(j) as *const __m256i)), ones),
            );
            acc1 = _mm256_add_epi32(
                acc1,
                _mm256_madd_epi16(_mm256_maddubs_epi16(x, _mm256_loadu_si256(w1.add(j) as *const __m256i)), ones),
            );
            acc2 = _mm256_add_epi32(
                acc2,
                _mm256_madd_epi16(_mm256_maddubs_epi16(x, _mm256_loadu_si256(w2.add(j) as *const __m256i)), ones),
            );
            acc3 = _mm256_add_epi32(
                acc3,
                _mm256_madd_epi16(_mm256_maddubs_epi16(x, _mm256_loadu_si256(w3.add(j) as *const __m256i)), ones),
            );
            j += 32;
        }
        out[r] = clip_relu(hsum256_epi32(acc0) + bias[r]);
        out[r + 1] = clip_relu(hsum256_epi32(acc1) + bias[r + 1]);
        out[r + 2] = clip_relu(hsum256_epi32(acc2) + bias[r + 2]);
        out[r + 3] = clip_relu(hsum256_epi32(acc3) + bias[r + 3]);
    }
}

#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "avx2")]
unsafe fn add_i16x512_avx2(dst: &mut [i16; 512], src: &[i16], sign: i16) {
    use std::arch::x86_64::*;
    let dp = dst.as_mut_ptr();
    let sp = src.as_ptr();
    let mut i = 0;
    if sign >= 0 {
        while i < 512 {
            let a = _mm256_loadu_si256(dp.add(i) as *const __m256i);
            let b = _mm256_loadu_si256(sp.add(i) as *const __m256i);
            _mm256_storeu_si256(dp.add(i) as *mut __m256i, _mm256_add_epi16(a, b));
            i += 16;
        }
    } else {
        while i < 512 {
            let a = _mm256_loadu_si256(dp.add(i) as *const __m256i);
            let b = _mm256_loadu_si256(sp.add(i) as *const __m256i);
            _mm256_storeu_si256(dp.add(i) as *mut __m256i, _mm256_sub_epi16(a, b));
            i += 16;
        }
    }
}

#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "avx2")]
unsafe fn fc1_relu_avx2(
    weights: &[i8],
    input: &[u8; 16],
    bias: &[i32; 32],
    out: &mut [u8; 32],
) {
    use std::arch::x86_64::*;
    let x = _mm256_cvtepu8_epi16(_mm_loadu_si128(input.as_ptr() as *const __m128i));
    let ones = _mm256_set1_epi16(1);
    for i in 0..32 {
        let w = _mm256_cvtepi8_epi16(_mm_loadu_si128(
            weights.as_ptr().add(i * 32) as *const __m128i,
        ));
        let products = _mm256_mullo_epi16(x, w);
        let sums = _mm256_madd_epi16(products, ones);
        out[i] = clip_relu(hsum256_epi32(sums) + bias[i]);
    }
}

#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "avx2")]
unsafe fn fc2_avx2(weights: &[i8; 32], input: &[u8; 32], bias: i32) -> i32 {
    use std::arch::x86_64::*;
    let x = _mm256_loadu_si256(input.as_ptr() as *const __m256i);
    let w = _mm256_loadu_si256(weights.as_ptr() as *const __m256i);
    let products = _mm256_maddubs_epi16(x, w);
    let sums = _mm256_madd_epi16(products, _mm256_set1_epi16(1));
    bias + hsum256_epi32(sums)
}

#[cfg(target_arch = "x86_64")]
#[inline]
#[target_feature(enable = "avx2")]
unsafe fn hsum256_epi32(v: std::arch::x86_64::__m256i) -> i32 {
    use std::arch::x86_64::*;
    let hi = _mm256_extracti128_si256::<1>(v);
    let lo = _mm256_castsi256_si128(v);
    let sum = _mm_add_epi32(lo, hi);
    let hi64 = _mm_unpackhi_epi64(sum, sum);
    let sum = _mm_add_epi32(sum, hi64);
    let hi32 = _mm_shuffle_epi32::<0x01>(sum);
    let sum = _mm_add_epi32(sum, hi32);
    _mm_cvtsi128_si32(sum)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fc0_and_clamp_match_scalar() {
        let mut weights = vec![0i8; 16 * 1024];
        let mut input = [0u8; 1024];
        let mut bias = [0i32; 16];
        for i in 0..weights.len() {
            weights[i] = (i as i32 * 13 - 40) as i8;
        }
        for i in 0..1024 {
            input[i] = (i % 128) as u8;
        }
        for i in 0..16 {
            bias[i] = i as i32 * 17 - 50;
        }

        let mut scalar = [0u8; 16];
        fc0_relu_scalar(&weights, &input, &bias, &mut scalar);
        let mut dispatched = [0u8; 16];
        fc0_relu(&weights, &input, &bias, &mut dispatched);
        assert_eq!(scalar, dispatched);

        let mut acc = [0i16; 512];
        for i in 0..512 {
            acc[i] = (i as i16).wrapping_mul(19).wrapping_sub(200);
        }
        let mut out_s = [0u8; 512];
        let mut out_d = [0u8; 512];
        clamp_acc_scalar(&acc, &mut out_s);
        clamp_acc(&acc, &mut out_d);
        assert_eq!(out_s.as_slice(), out_d.as_slice());

        let mut dst_s = acc;
        let mut dst_d = acc;
        let src = acc;
        add_i16x512_scalar(&mut dst_s, &src, -1);
        add_i16x512(&mut dst_d, &src, -1);
        assert_eq!(dst_s, dst_d);
        add_i16x512_scalar(&mut dst_s, &src, 1);
        add_i16x512(&mut dst_d, &src, 1);
        assert_eq!(dst_s, dst_d);

        let mut h1 = [0u8; 16];
        let mut w1 = vec![0i8; 32 * 32];
        let mut b1 = [0i32; 32];
        for i in 0..16 {
            h1[i] = (i * 7) as u8;
        }
        for i in 0..w1.len() {
            w1[i] = ((i as i32 * 11 % 127) - 63) as i8;
        }
        for i in 0..32 {
            b1[i] = i as i32 * 31 - 400;
        }
        let mut h2s = [0u8; 32];
        let mut h2d = [0u8; 32];
        fc1_relu_scalar(&w1, &h1, &b1, &mut h2s);
        fc1_relu(&w1, &h1, &b1, &mut h2d);
        assert_eq!(h2s, h2d);

        let mut w2 = [0i8; 32];
        for i in 0..32 {
            w2[i] = i as i8 - 16;
        }
        assert_eq!(fc2_scalar(&w2, &h2s, 123), fc2(&w2, &h2s, 123));
    }
}
