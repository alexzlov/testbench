extern crate cpp_build;
extern crate gcc;
fn main() {
    cpp_build::build("src/main.rs");
    gcc::Build::new()
        .file("imgui/imgui.cpp")
        .file("imgui/imgui_draw.cpp")
        .compile("libimgui.lib");
}