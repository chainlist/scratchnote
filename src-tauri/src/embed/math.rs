//! The arithmetic on vectors that embedding, threads and the map share.

/// Summed in eight lanes, which the compiler turns into vector
/// instructions: one running sum would make each addition wait on the last.
pub fn dot(a: &[f32], b: &[f32]) -> f32 {
    let len = a.len().min(b.len());
    let (a, b) = (&a[..len], &b[..len]);
    let mut lanes = [0.0f32; 8];
    let (a8, b8) = (a.chunks_exact(8), b.chunks_exact(8));
    let tail: f32 = a8
        .remainder()
        .iter()
        .zip(b8.remainder())
        .map(|(x, y)| x * y)
        .sum();
    for (x, y) in a8.zip(b8) {
        for i in 0..8 {
            lanes[i] += x[i] * y[i];
        }
    }
    lanes.iter().sum::<f32>() + tail
}

/// Adds `vector` times `sign` into `sum`, kept in f64 so that years of
/// adding and taking off do not build up rounding.
pub fn add(sum: &mut [f64], vector: &[f32], sign: f64) {
    for (s, &x) in sum.iter_mut().zip(vector) {
        *s += sign * x as f64;
    }
}

/// Adds `vector` into `sum`.
pub fn add_into(sum: &mut [f32], vector: &[f32]) {
    for (s, x) in sum.iter_mut().zip(vector) {
        *s += x;
    }
}

/// Scale to unit length, so similarity is a plain dot product. A zero vector
/// has no direction and is left as it is: it scores zero against anything.
pub fn normalize(vector: &mut [f32]) {
    let norm = vector.iter().map(|x| x * x).sum::<f32>().sqrt();
    if norm > 0.0 {
        for x in vector {
            *x /= norm;
        }
    }
}

/// `vector` as little-endian bytes, four to a value, as `space.db` and
/// `vectors.bin` keep it.
pub fn to_le_bytes(vector: &[f32]) -> Vec<u8> {
    vector.iter().flat_map(|x| x.to_le_bytes()).collect()
}

/// The values `to_le_bytes` wrote. Bytes past the last whole value are
/// left out.
pub fn from_le_bytes(bytes: &[u8]) -> Vec<f32> {
    bytes
        .chunks_exact(4)
        .map(|b| f32::from_le_bytes([b[0], b[1], b[2], b[3]]))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bytes_round_trip_and_sums_add_up() {
        let vector = vec![1.5, -2.0, 0.25];
        assert_eq!(from_le_bytes(&to_le_bytes(&vector)), vector);
        assert_eq!(dot(&vector, &vector), 1.5 * 1.5 + 4.0 + 0.0625);

        let mut sum = vec![0.0f64; 3];
        add(&mut sum, &vector, 1.0);
        add(&mut sum, &vector, -1.0);
        assert_eq!(sum, vec![0.0; 3]);
        let mut total = vec![1.0f32; 3];
        add_into(&mut total, &vector);
        assert_eq!(total, vec![2.5, -1.0, 1.25]);

        let mut unit = vec![3.0, 4.0];
        normalize(&mut unit);
        assert_eq!(unit, vec![0.6, 0.8]);
    }
}
