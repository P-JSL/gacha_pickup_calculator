//! O(N log N) Radix-2 FFT for dense first-passage distributions.
//! Non-probability, sparse, short or numerically unstable cases fall back to
//! the original direct convolution so supported input semantics are preserved.
use std::f64::consts::PI;

#[derive(Clone, Copy, Default)]
struct Complex { re: f64, im: f64 }

fn fft(buf: &mut [Complex], inverse: bool) {
    let n = buf.len();
    let mut j = 0;
    for i in 1..n {
        let mut bit = n >> 1;
        while (j & bit) != 0 {
            j ^= bit;
            bit >>= 1;
        }
        j ^= bit;
        if i < j { buf.swap(i, j); }
    }
    let mut width = 2;
    while width <= n {
        let theta = 2.0 * PI / width as f64 * if inverse { 1.0 } else { -1.0 };
        let (sin, cos) = theta.sin_cos();
        for start in (0..n).step_by(width) {
            let (mut wr, mut wi) = (1.0, 0.0);
            for k in 0..width/2 {
                let u = buf[start+k];
                let v = buf[start+k+width/2];
                let (vr, vi) = (v.re*wr-v.im*wi, v.re*wi+v.im*wr);
                buf[start+k] = Complex { re:u.re+vr, im:u.im+vi };
                buf[start+k+width/2] = Complex { re:u.re-vr, im:u.im-vi };
                let next_wr = wr*cos - wi*sin;
                wi = wr*sin + wi*cos;
                wr = next_wr;
            }
        }
        if width == n { break; }
        width *= 2;
    }
    if inverse {
        let scale = 1.0 / n as f64;
        for v in buf { v.re *= scale; v.im *= scale; }
    }
}

/// Dense convolution runs in O(N log N) time and O(N) extra memory.
/// Return None when direct summation is preferable or its safety checks fail.
pub(crate) fn convolve_dense(a: &[f64], b: &[f64]) -> Option<Vec<f64>> {
    let output_len = a.len().checked_add(b.len())?.checked_sub(1)?;
    if a.len().saturating_mul(b.len()) < 32_768 { return None; }
    if a.iter().chain(b).any(|&x| !x.is_finite() || x < 0.0) { return None; }
    let n = output_len.checked_next_power_of_two()?;
    let nz_a = a.iter().filter(|&&x| x != 0.0).count();
    let nz_b = b.iter().filter(|&&x| x != 0.0).count();
    if nz_a.saturating_mul(nz_b) <= n.saturating_mul(n.ilog2() as usize).saturating_mul(3) {
        return None;
    }
    let mut x = vec![Complex::default();n];
    let mut y = vec![Complex::default();n];
    for (v,&p) in x.iter_mut().zip(a) { v.re=p; }
    for (v,&p) in y.iter_mut().zip(b) { v.re=p; }
    fft(&mut x,false);
    fft(&mut y,false);
    for (u,v) in x.iter_mut().zip(y) {
        *u=Complex{ re:u.re*v.re-u.im*v.im, im:u.re*v.im+u.im*v.re };
    }
    fft(&mut x,true);
    let mut out=Vec::with_capacity(output_len);
    for v in x.into_iter().take(output_len) {
        if v.re < -1e-9 || !v.re.is_finite() || v.im.abs() > 1e-7 { return None; }
        out.push(v.re.max(0.0));
    }
    let expected=a.iter().sum::<f64>()*b.iter().sum::<f64>();
    if (out.iter().sum::<f64>()-expected).abs() > 1e-8*expected.abs().max(1.0) {
        return None;
    }
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dense_fft_matches_direct_convolution_and_preserves_mass() {
        for (left, right) in [(257, 513), (512, 512), (1023, 259)] {
            let mut a=(0..left).map(|i| (i%11+1) as f64).collect::<Vec<_>>();
            let mut b=(0..right).map(|i| (i%17+1) as f64).collect::<Vec<_>>();
            let sa=a.iter().sum::<f64>();
            let sb=b.iter().sum::<f64>();
            for v in &mut a {*v/=sa;}
            for v in &mut b {*v/=sb;}
            let actual=convolve_dense(&a,&b).expect("dense FFT expected");
            let mut expected=vec![0.0;left+right-1];
            for (i,&ai) in a.iter().enumerate() {
                for (j,&bj) in b.iter().enumerate(){expected[i+j]+=ai*bj;}
            }
            assert_eq!(actual.len(), expected.len());
            for (i,(x,y)) in actual.iter().zip(expected.iter()).enumerate() {
                assert!((x-y).abs() < 1e-9, "FFT error at {i}: {x} vs {y}");
            }
            assert!((actual.iter().sum::<f64>() - 1.0).abs() < 1e-8);
        }
    }

    #[test]
    fn invalid_and_sparse_inputs_keep_direct_path() {
        assert!(convolve_dense(&[1.0, -1.0], &[1.0, 2.0]).is_none());
        assert!(convolve_dense(&[1.0, f64::NAN], &[1.0, 2.0]).is_none());
        let a=vec![0.0;500];
        assert!(convolve_dense(&a,&a).is_none());
    }
}
