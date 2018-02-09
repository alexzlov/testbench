extern crate num;
extern crate minifb;
extern crate crossbeam;

use minifb::{Key, WindowOptions, Window};
use num::Complex;


const WIDTH:  usize = 1024;
const HEIGHT: usize = 768;

fn pixel_to_point(bounds:      (usize, usize),
                  pixel:       (usize, usize),
                  upper_left:  Complex<f64>,
                  lower_right: Complex<f64>) -> Complex<f64> {

    let (width, height) = (lower_right.re - upper_left.re,
                           upper_left.im  - lower_right.im);
    Complex {
        re: upper_left.re + pixel.0 as f64 * width  / bounds.0 as f64,
        im: upper_left.im - pixel.1 as f64 * height / bounds.1 as f64
    }
}

fn estimate(c: Complex<f64>, limit: u32) -> Option<u32> {
    let mut z = Complex {re: 0.0, im: 0.0};
    for i in 0 .. limit {
        z = z * z + c;
        if z.norm_sqr() > 4.0 {
            return Some(i);
        }
    }
    None
}

fn render(pixels:      &mut [u32],
          bounds:      (usize, usize),
          upper_left:  Complex<f64>,
          lower_right: Complex<f64>) {

    assert!(pixels.len() == bounds.0 * bounds.1);
    for row in 0 .. bounds.1 {
        for column in 0 .. bounds.0 {
            let point = pixel_to_point(bounds, (column, row), upper_left, lower_right);
            pixels[row * bounds.0 + column] = match estimate(point, 255) {
                None        => 0,
                Some(count) => (255 - count) << 16
            };
        }
    }
}

fn render_parallel(pixels:      &mut Vec<u32>,
                   bounds:      (usize, usize),
                   upper_left:  Complex<f64>,
                   lower_right: 
                   Complex<f64>) {

    let threads = 4;
    let rows_per_band = bounds.1 / threads + 1;
    let bands: Vec<&mut [u32]> = pixels.chunks_mut(rows_per_band * bounds.0).collect();
    crossbeam::scope(|spawner| {
        for (i, band) in bands.into_iter().enumerate() {
            let top              = rows_per_band * i;
            let height           = band.len() / bounds.0;
            let band_bounds      = (bounds.0, height);
            let band_upper_left  = pixel_to_point(bounds, (0, top), upper_left, lower_right);
            let band_lower_right = pixel_to_point(bounds, (bounds.0, top + height), upper_left, lower_right);
            spawner.spawn(move || {
                render(band, band_bounds, band_upper_left, band_lower_right);
            });
        }
    });
}

fn main() {
    let mut buffer: Vec<u32> = vec![0; WIDTH * HEIGHT];
    let mut window = Window::new(
        "Sample RGBA32 buffer", WIDTH, HEIGHT, WindowOptions::default()
    ).unwrap_or_else(|e| { panic!("{}", e); });
    let mut upper_left  = Complex {re: -1.20, im: 0.35};
    let mut lower_right = Complex {re: -1.0,  im: 0.20};
    render(&mut buffer, (WIDTH, HEIGHT), upper_left, lower_right);
    while window.is_open() && !window.is_key_down(Key::Escape) {
        if window.is_key_down(Key::Left) {
            upper_left.re  -= 0.01;
            lower_right.re -= 0.01;
            render_parallel(&mut buffer, (WIDTH, HEIGHT), upper_left, lower_right);
        }
        if window.is_key_down(Key::Right) {
            upper_left.re  += 0.01;
            lower_right.re += 0.01;
            render_parallel(&mut buffer, (WIDTH, HEIGHT), upper_left, lower_right);
        }
        if window.is_key_down(Key::Up) {
            upper_left.im  += 0.01;
            lower_right.im += 0.01;
            render_parallel(&mut buffer, (WIDTH, HEIGHT), upper_left, lower_right);
        }
        if window.is_key_down(Key::Down) {
            upper_left.im  -= 0.01;
            lower_right.im -= 0.01;
            render_parallel(&mut buffer, (WIDTH, HEIGHT), upper_left, lower_right);
        }
        window.update_with_buffer(&buffer).unwrap();
    }
}
