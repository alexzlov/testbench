extern crate num;
extern crate minifb;
extern crate crossbeam;
extern crate simd;

use minifb::{Key, WindowOptions, Window};
use num::Complex;
use simd::{f32x4, u32x4};

const WIDTH:  usize = 1024;
const HEIGHT: usize = 768;
const COLORS: &'static [(f32, f32, f32)] = &[(0.0,    7.0,    100.0),
                                             (32.0,   107.0,  203.0),
                                             (237.0,  255.0,  255.0),
                                             (255.0,  170.0,  0.0),
                                             (0.0,    2.0,    0.0)];

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
            let (r, g, b);
            pixels[row * bounds.0 + column] = match estimate(point, 255) {
                None        => 0,
                Some(count) => {
                    let val   = (count as f32 % 12.0) * (COLORS.len() as f32) / 12.0;
                    let left  = val as usize % COLORS.len();
                    let right = (left + 1) % COLORS.len();

                    let p = val - left as f32;
                    let (r1, g1, b1) = COLORS[left];
                    let (r2, g2, b2) = COLORS[right];
                    r = (r1 + (r2 - r1) * p) as u32;
                    g = (g1 + (g2 - g1) * p) as u32;
                    b = (b1 + (b2 - b1) * p) as u32;
                    (r << 16) + (g << 8) + b
                }
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

    // Bounds to see the whole picture:
    // let mut upper_left  = Complex {re: -2.2, im:  1.0};
    // let mut lower_right = Complex {re:  1.2, im: -1.0};
    
    let mut upper_left  = Complex {re: -1.20, im: 0.35};
    let mut lower_right = Complex {re: -1.0,  im: 0.20};
    render(&mut buffer, (WIDTH, HEIGHT), upper_left, lower_right);
    while window.is_open() && !window.is_key_down(Key::Escape) {
        let mut need_update = false;
        window.get_keys().map(|keys| {
            for k in keys {
                match k {
                    Key::Left  => {upper_left.re -= 0.01; lower_right.re -= 0.01;},
                    Key::Right => {upper_left.re += 0.01; lower_right.re += 0.01;},
                    Key::Up    => {upper_left.im += 0.01; lower_right.im += 0.01;},
                    Key::Down  => {upper_left.im -= 0.01; lower_right.im -= 0.01;},
                    _          => (),
                }
                need_update = true;
            }
            if need_update {
                render_parallel(&mut buffer, (WIDTH, HEIGHT), upper_left, lower_right);
                need_update = false;
            }
        });
        window.update_with_buffer(&buffer).unwrap();
    }
}
