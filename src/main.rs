extern crate num;
extern crate minifb;

use minifb::{Key, WindowOptions, Window};

use num::Complex;
use std::str::FromStr;

const WIDTH:  usize = 640;
const HEIGHT: usize = 360;

#[allow(dead_code)]
fn complex_square_add_loop(c: Complex<f64>, limit: u32) -> Option<u32> {
    let mut z = Complex { re: 0.0, im: 0.0 };
    for i in 0..limit {
        z = z * z + c;
        if z.norm_sqr() > 4.0 {
            return Some(i);
        }
    }
    None
}

fn parse_pair<T: FromStr>(s: &str, separator: char) -> Option<(T, T)> {
    match s.find(separator) {
        None => None,
        Some(index) => {
            match (T::from_str(&s[..index]), T::from_str(&s[index + 1..])) {
                (Ok(l), Ok(r)) => Some((l, r)),
                _              => None
            }
        }
    }
}

fn main() {
    let mut buffer: Vec<u32> = vec![0; 640 * 360];
    let mut window = Window::new(
        "Sample RGBA32 buffer", WIDTH, HEIGHT, WindowOptions::default()
    ).unwrap_or_else(|e| { panic!("{}", e); });
    while window.is_open() && !window.is_key_down(Key::Escape) {
        for i in buffer.iter_mut() {
            *i = 127 << 16; // 16 - R, 0 - B, 8 - G
            *i = 126;
        }
        window.update_with_buffer(&buffer).unwrap();
    }
}
