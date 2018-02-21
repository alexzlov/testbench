#![feature(iterator_step_by, test)]

#[macro_use]
extern crate cpp;
#[macro_use]
extern crate lazy_static;

cpp!{{
    #include <stdio.h>
    #include "imgui/imgui.h"
}}


extern crate num;
extern crate minifb;
extern crate crossbeam;
extern crate simd;
extern crate libc;

use minifb::{Key, WindowOptions, Window};
use num::Complex;
use simd::{f32x4, u32x4};
use std::sync::Mutex;

const WIDTH:       usize = 1024;
const HEIGHT:      usize = 768;
const LIMIT:       u32   = 100;
const NUM_THREADS: usize = 4;
const COLORS: &'static [(f32, f32, f32)] = &[(0.0,    7.0,    100.0),
                                             (32.0,   107.0,  203.0),
                                             (237.0,  255.0,  255.0),
                                             (255.0,  170.0,  0.0),
                                             (0.0,    2.0,    0.0)];

#[repr(C)]
struct Point2D {
    x: i32,
    y: i32
}

#[repr(C)]
struct Point2DF {
    x: f32,
    y: f32,
}

lazy_static! {
    static ref GlobalBuffer: Mutex<Vec<u32>> = Mutex::new(vec![0; WIDTH * HEIGHT]);
}

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

#[inline(never)]
fn mandelbrot_vector(c_x: f32x4, c_y: f32x4, max_iter: u32) -> u32x4 {
    let mut x = c_x;
    let mut y = c_y;
    let mut count = u32x4::splat(0);
    for _ in 0..max_iter as usize {
        let xy = x * y;
        let xx = x * x;
        let yy = y * y;
        let sum = xx + yy;
        let mask = sum.lt(f32x4::splat(4.0));
        if !mask.any() { break }
        count = count + mask.to_i().select(u32x4::splat(1), u32x4::splat(0));
        x = xx - yy + c_x;
        y = xy + xy + c_y;
    }
    count
}

#[inline(never)]
fn render(pixels:      &mut [u32],
          bounds:      (usize, usize),
          upper_left:  Complex<f64>,
          lower_right: Complex<f64>) {

    assert!(pixels.len() == bounds.0 * bounds.1);

    let left             = upper_left.re  as f32;
    let right            = lower_right.re as f32;
    let top              = upper_left.im  as f32;
    let bottom           = lower_right.im as f32;
    let width_step:  f32 = (right - left) / WIDTH as f32;
    let height_step: f32 = (bottom - top) / (HEIGHT as f32 / NUM_THREADS as f32) ;
    let adjust           = f32x4::splat(width_step) * f32x4::new(0., 1., 2., 3.);

    for row in 0 .. bounds.1 {
        let y = f32x4::splat(top + height_step * row as f32);
        for column in (0 .. bounds.0).step_by(4) {
            let x = f32x4::splat(left + width_step * column as f32) + adjust;
            let points = mandelbrot_vector(x, y, LIMIT);            
            for k in 0..4 {
                let (r, g, b);
                let point_k = points.extract(k as u32);
                let val   = (point_k as f32 % 12.0) * (COLORS.len() as f32) / 12.0;
                let left  = val as usize % COLORS.len();
                let right = (left + 1) % COLORS.len();

                let p = val - left as f32;
                let (r1, g1, b1) = COLORS[left];
                let (r2, g2, b2) = COLORS[right];
                r = (r1 + (r2 - r1) * p) as u32;
                g = (g1 + (g2 - g1) * p) as u32;
                b = (b1 + (b2 - b1) * p) as u32;
                pixels[row * bounds.0 + column + k] = (r << 16) + (g << 8) + b
            }            
        }
    }
}

fn render_parallel(bounds:      (usize, usize),
                   upper_left:  Complex<f64>,
                   lower_right: Complex<f64>) {
    let rows_per_band = bounds.1 / NUM_THREADS + 1;
    let mut buffer_contents = GlobalBuffer.lock().unwrap();
    let bands: Vec<&mut [u32]> = buffer_contents.chunks_mut(rows_per_band * bounds.0).collect();
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

#[inline]
fn edge_function(p0: &Point2DF, p1: &Point2DF, p2: &Point2DF) -> f32 {
    (p1.x - p0.x) * (p2.y - p0.y) - (p1.y - p0.y) * (p2.x - p0.x)
}

#[inline]
fn min3(x: f32, y: f32, z: f32) -> f32 {
    let mut min = x;
    if y < min { min = y; }
    if z < min { min = z; }
    min as f32
}

#[inline]
fn max3(x: f32, y: f32, z: f32) -> f32 {
    let mut max = x;
    if y > max { max = y; }
    if z > max { max = z; }
    max as f32
}

fn draw_triangle(p0: &Point2DF, p1: &Point2DF, p2: &Point2DF,
                 R0: f32, G0: f32, B0: f32, A0: f32,
                 R1: f32, G1: f32, B1: f32, A1: f32,
                 R2: f32, G2: f32, B2: f32, A2: f32,
                 uv0: &Point2DF, uv1: &Point2DF, uv2: &Point2DF) {
    let area = edge_function(&p0, &p1, &p2);
    let min_x = min3(p0.x, p1.x, p2.x);
    let max_x = max3(p0.x, p1.x, p2.x);
    let min_y = min3(p0.y, p1.y, p2.y);
    let max_y = max3(p0.y, p1.y, p2.y);

    let mut p_y = min_y.ceil() as i32;
    let mut p_x = min_x.ceil() as i32;
    for y in min_y.ceil() as i32 .. max_y.ceil() as i32 {
        for x in min_x.ceil() as i32 .. max_x.ceil() as i32 {
            let p = Point2DF {x: x as f32, y: y as f32};
            let mut w0 = edge_function(&p1, &p2, &p);
            let mut w1 = edge_function(&p2, &p0, &p);
            let mut w2 = edge_function(&p0, &p1, &p);

            if (w0 >= 0.0 && w1 >= 0.0 && w2 >= 0.0) {
                w0 /= area;
                w1 /= area;
                w2 /= area;

                let MeshR = ((w0 * R0 + w1 * R1 + w2 * R2) * 255.0) as u32;
                let MeshG = ((w0 * G0 + w1 * G1 + w2 * G2) * 255.0) as u32;
                let MeshB = ((w0 * B0 + w1 * B1 + w2 * B2) * 255.0) as u32;
                let MeshA = ((w0 * A0 + w1 * A1 + w2 * A2) * 255.0) as u32;
                let mut pixels = GlobalBuffer.lock().unwrap();
                pixels[y as usize * WIDTH + x as usize] = (MeshR << 16) + (MeshG << 8) + MeshB;
            }
        }
    }
}

fn fetch_render_data(_im_draw_data: *const ()) {
    let rasterizer = draw_triangle as *const ();
    unsafe {
        cpp!([_im_draw_data as "void *", rasterizer as "void *"] {             
            
            struct Point2DF {
                float X;
                float Y;
            };

            typedef void DrawTriangle(Point2DF* p0, Point2DF* p1, Point2DF* p2,
                                      float R0, float G0, float B0, float A0,
                                      float R1, float G1, float B1, float A1,
                                      float R2, float G2, float B2, float A2,
                                      Point2DF* uv0, Point2DF* uv1, Point2DF* uv2);      
            DrawTriangle *rusterizer = (DrawTriangle *) rasterizer;
            ImGuiIO& io = ImGui::GetIO();
            int fb_width  = (int)(io.DisplaySize.x * io.DisplayFramebufferScale.x);
            int fb_height = (int)(io.DisplaySize.y * io.DisplayFramebufferScale.y);
            if (fb_width == 0 || fb_height == 0) {
                printf("Skip frame\n");
                return;
            }
            ImDrawData *data = (ImDrawData *)_im_draw_data;
            data->ScaleClipRects(io.DisplayFramebufferScale);
            for (int n = 0; n < data->CmdListsCount; n++) {
                const ImDrawList *cmd_list = data->CmdLists[n];
                unsigned int IndexOffset = 0;
                for (int cmd_i = 0; cmd_i < cmd_list->CmdBuffer.Size; cmd_i++) {
                    const ImDrawCmd *pcmd = &cmd_list->CmdBuffer[cmd_i];
                    unsigned int ElementCount = (unsigned int)pcmd->ElemCount;
                    if (pcmd->UserCallback) {
                        printf("User input is not implemented.\n");
                    } else {
                        for (unsigned int i = 0; i < ElementCount; i+= 3) {
                            unsigned int idx0 = cmd_list->IdxBuffer[IndexOffset + i];
                            unsigned int idx1 = cmd_list->IdxBuffer[IndexOffset + i + 1];
                            unsigned int idx2 = cmd_list->IdxBuffer[IndexOffset + i + 2];

                            Point2DF p0 = {cmd_list->VtxBuffer[idx0].pos.x,
                                           cmd_list->VtxBuffer[idx0].pos.y};
                            Point2DF p1 = {cmd_list->VtxBuffer[idx1].pos.x,
                                           cmd_list->VtxBuffer[idx1].pos.y};
                            Point2DF p2 = {cmd_list->VtxBuffer[idx2].pos.x,
                                           cmd_list->VtxBuffer[idx2].pos.y};

                            Point2DF uv0 = {cmd_list->VtxBuffer[idx0].uv.x,
                                            cmd_list->VtxBuffer[idx0].uv.y};
                            Point2DF uv1 = {cmd_list->VtxBuffer[idx1].uv.x,
                                            cmd_list->VtxBuffer[idx1].uv.y};
                            Point2DF uv2 = {cmd_list->VtxBuffer[idx2].uv.x,
                                            cmd_list->VtxBuffer[idx2].uv.y};
                            
                            ImVec4 rgba0 = ImGui::ColorConvertU32ToFloat4(cmd_list->VtxBuffer[idx0].col);
                            ImVec4 rgba1 = ImGui::ColorConvertU32ToFloat4(cmd_list->VtxBuffer[idx1].col);
                            ImVec4 rgba2 = ImGui::ColorConvertU32ToFloat4(cmd_list->VtxBuffer[idx2].col);
                            rusterizer(&p0, &p1, &p2,
                                       rgba0.x, rgba0.y, rgba0.z, rgba0.w,
                                       rgba1.x, rgba1.y, rgba1.z, rgba1.w,
                                       rgba2.x, rgba2.y, rgba2.z, rgba2.w,
                                       &uv0, &uv1, &uv2);
                        }
                    }
                    IndexOffset += ElementCount;
                }
            }
        });
    }    
}

fn init_imgui() {
    unsafe {
        let w = WIDTH  as u32;
        let h = HEIGHT as u32;
        let renderer = fetch_render_data as *const ();
        cpp!([w as "int32_t", h as "int32_t", renderer as "void *"] {
            typedef void rust_renderer(ImDrawData *data);
            printf("Starting imgui initialization...\n");
            ImGui::CreateContext();
            ImGuiIO& io = ImGui::GetIO();
            io.RenderDrawListsFn = (rust_renderer*)renderer;
            io.DisplaySize = ImVec2((float)w, (float)h);               
            unsigned char *font_texture = NULL;
            int tex_w, tex_h, tex_bpp;
            io.Fonts->GetTexDataAsAlpha8(&font_texture, &tex_w, &tex_h, &tex_bpp);
            printf("OK: Finishing imgui initialization.\n");
        });
    }
}

fn main() {
    let mut window = Window::new(
        "Sample RGBA32 buffer", WIDTH, HEIGHT, WindowOptions::default()
    ).unwrap_or_else(|e| { panic!("{}", e); });       

    println!("Renderer version: 0.0.666, x86_64, AVX2");
    println!("========================================");
    println!("Running with {} threads.", NUM_THREADS);
    println!("Buffer resolution: {} - {}.", WIDTH, HEIGHT);
    
    init_imgui();

    let mut upper_left  = Complex {re: -2.2, im:  1.0};
    let mut lower_right = Complex {re:  1.2, im: -1.0};
    let mut step        = 0.01;
    render_parallel((WIDTH, HEIGHT), upper_left, lower_right);

    while window.is_open() && !window.is_key_down(Key::Escape) {
        let mut need_update = false;
        let w = WIDTH  as u32;
        let h = HEIGHT as u32;
        unsafe {   
            cpp!([w as "int32_t", h as "int32_t"] {
                ImGuiIO& io = ImGui::GetIO();
                io.DisplaySize = ImVec2(w, h); 
                ImGui::NewFrame();
                ImGui::Begin("Stats", 0);
                ImGui::SetWindowPos("Stats", ImVec2(10, 10));
                ImGui::SetWindowSize(ImVec2(300, 85));
                
                ImGui::PushStyleColor(ImGuiCol_Text, ImVec4(1.0f, 0.2f, 0.2f, 1.0f));
                ImGui::Text("Milliseconds per frame: ");
                ImGui::PopStyleColor();

                ImGui::End();
                ImGui::Render();
                ImGui::EndFrame();
            });
        }
        window.get_keys().map(|keys| {            
            for k in keys {
                match k {
                    Key::Left  => {upper_left.re -= step; lower_right.re -= step;},
                    Key::A     => {upper_left.re -= step; lower_right.re -= step;},
                    Key::Right => {upper_left.re += step; lower_right.re += step;},
                    Key::D     => {upper_left.re += step; lower_right.re += step;},
                    Key::Up    => {upper_left.im += step; lower_right.im += step;},
                    Key::Down  => {upper_left.im -= step; lower_right.im -= step;},
                    Key::W     => {upper_left.re /= 1.01; lower_right.re /= 1.01;
                                   upper_left.im /= 1.01; lower_right.im /= 1.01;
                                   step          /= 1.01;},
                    Key::S     => {upper_left.re *= 1.01; lower_right.re *= 1.01;
                                   upper_left.im *= 1.01; lower_right.im *= 1.01;
                                   step          *= 1.01},
                    _          => (),
                }
                need_update = true;
            }
            if need_update {
                render_parallel((WIDTH, HEIGHT), upper_left, lower_right);                              
                need_update = false;                
            }
        });
        window.update_with_buffer(&GlobalBuffer.lock().unwrap()).unwrap();
    }
}
