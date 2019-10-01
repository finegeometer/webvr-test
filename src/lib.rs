#![forbid(unsafe_code)]

use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;

fn to_js_closure(
    f: impl FnOnce(JsValue) -> Result<(), JsValue> + 'static,
) -> Closure<dyn FnMut(JsValue)> {
    Closure::once(|val| f(val).unwrap_throw())
}

/// Returns `|t| f(f, t)` as a JS function.
fn self_referential_function<T: 'static + wasm_bindgen::convert::FromWasmAbi>(
    mut f: impl FnMut(js_sys::Function, T) -> Result<(), JsValue> + 'static,
) -> js_sys::Function {
    use std::cell::RefCell;
    use std::rc::Rc;

    #[allow(clippy::type_complexity)]
    let function_1: Rc<RefCell<Option<Closure<dyn FnMut(T)>>>> = Rc::new(RefCell::new(None));
    let function_2 = function_1.clone();

    let closure = move |t| {
        f(
            function_1
                .borrow()
                .as_ref()
                .unwrap_throw()
                .as_ref()
                .unchecked_ref::<js_sys::Function>()
                .clone(),
            t,
        )
        .unwrap_throw();
    };

    *function_2.borrow_mut() = Some(Closure::wrap(Box::new(closure)));

    let out: js_sys::Function = function_2
        .borrow()
        .as_ref()
        .unwrap_throw()
        .as_ref()
        .unchecked_ref::<js_sys::Function>()
        .clone();
    out
}

#[wasm_bindgen]
pub fn run() -> Result<(), JsValue> {
    std::panic::set_hook(Box::new(console_error_panic_hook::hook));
    web_sys::console::log_1(&"Test Count: 7".into()); // Increment on each test, so I know when GH pages updates.

    let window = web_sys::window().ok_or("no global `window` exists")?;
    let document = window
        .document()
        .ok_or("should have a document on window")?;
    let body = document.body().ok_or("document should have a body")?;

    let canvas = document
        .create_element("canvas")?
        .dyn_into::<web_sys::HtmlCanvasElement>()?;
    canvas.set_attribute("width", "800")?;
    canvas.set_attribute("height", "800")?;
    body.append_child(&canvas)?;

    //

    type GL = web_sys::WebGl2RenderingContext;

    let gl = canvas
        .get_context("webgl2")?
        .ok_or("\"webgl2\" context identifier not supported.")?
        .dyn_into::<GL>()?;

    let program = gl.create_program().ok_or("Could not create program.")?;

    {
        let vertex_shader = gl
            .create_shader(GL::VERTEX_SHADER)
            .ok_or("Could not create shader")?;
        gl.shader_source(&vertex_shader, VERTEX_SHADER);
        gl.compile_shader(&vertex_shader);
        gl.attach_shader(&program, &vertex_shader);

        let fragment_shader = gl
            .create_shader(GL::FRAGMENT_SHADER)
            .ok_or("Could not create shader")?;
        gl.shader_source(&fragment_shader, FRAGMENT_SHADER);
        gl.compile_shader(&fragment_shader);
        gl.attach_shader(&program, &fragment_shader);

        gl.link_program(&program);
        gl.delete_shader(Some(&vertex_shader));
        gl.delete_shader(Some(&fragment_shader));
    }

    let pos_loc = gl.get_attrib_location(&program, "pos") as u32;

    let vao = gl
        .create_vertex_array()
        .ok_or("create_vertex_array failed")?;
    gl.bind_vertex_array(Some(&vao));

    let vertex_buffer = gl.create_buffer().ok_or("create_buffer failed")?;
    gl.bind_buffer(GL::ARRAY_BUFFER, Some(&vertex_buffer));
    gl.enable_vertex_attrib_array(pos_loc);
    gl.vertex_attrib_pointer_with_i32(pos_loc, 2, GL::FLOAT, false, 2 * 4, 0);

    let render_function = std::rc::Rc::new(move || -> std::result::Result<(), JsValue> {
        gl.clear_color(0., 0., 0., 1.);
        gl.clear(GL::COLOR_BUFFER_BIT);

        gl.bind_vertex_array(Some(&vao));

        gl.bind_buffer(GL::ARRAY_BUFFER, Some(&vertex_buffer));
        gl.buffer_data_with_array_buffer_view(
            GL::ARRAY_BUFFER,
            &as_f32_array(&[0.0, 0.0, 0.0, 1.0, 1.0, 1.0, 1.0, 1.0, 1.0, 0.0, 0.0, 0.0])?.into(),
            GL::STATIC_DRAW,
        );

        gl.use_program(Some(&program));

        gl.viewport(0, 0, 400, 800);
        gl.draw_arrays(GL::TRIANGLES, 0, 6);

        gl.viewport(400, 0, 400, 800);
        gl.draw_arrays(GL::TRIANGLES, 0, 6);

        Ok(())
    });

    //

    let navigator: web_sys::Navigator = window.navigator();

    let closure = to_js_closure(move |vr_displays: JsValue| {
        let render_function = render_function.clone();

        let vr_displays: js_sys::Array = js_sys::Array::from(&vr_displays);
        //
        if vr_displays.length() == 0 {
            return Err("No VR display".into());
        }

        let vr_display: web_sys::VrDisplay = vr_displays.get(0).dyn_into()?;

        //

        canvas.clone().add_event_listener_with_callback(
            "mousedown",
            &self_referential_function(move |this_function, _evt: web_sys::MouseEvent| {
                let render_function = render_function.clone();

                canvas.remove_event_listener_with_callback("mousedown", &this_function)?;

                canvas.request_pointer_lock();

                let mut layer = web_sys::VrLayer::new();
                layer.source(Some(&canvas));
                let layers = js_sys::Array::new();
                layers.set(0, layer.as_ref().clone());

                let vr_display = vr_display.clone();
                let vr_display_2 = vr_display.clone();
                let closure = to_js_closure(move |_| {
                    let render_function = render_function.clone();

                    vr_display
                        .clone()
                        .request_animation_frame(&self_referential_function(
                            move |this_function, _timestamp: f64| {
                                vr_display.request_animation_frame(&this_function)?;

                                render_function()?;

                                vr_display.submit_frame();

                                Ok(())
                            },
                        ))?;
                    Ok(())
                });

                vr_display_2.request_present(&layers)?.then(&closure);
                closure.forget();

                Ok(())
            }),
        )
    });

    navigator.get_vr_displays()?.then(&closure);
    closure.forget();

    Ok(())
}

const VERTEX_SHADER: &str = r#"#version 300 es

in vec2 pos;
out vec2 vpos;

void main() {
    vpos = pos;
    gl_Position = vec4(pos, 0.0, 1.0);
}

"#;

const FRAGMENT_SHADER: &str = r#"#version 300 es

precision mediump float;

in vec2 vpos;
out vec4 color;

void main() {
    color = vec4(vpos, 0.0, 1.0);
}

"#;

fn as_f32_array(v: &[f32]) -> Result<js_sys::Float32Array, JsValue> {
    let memory_buffer = wasm_bindgen::memory()
        .dyn_into::<js_sys::WebAssembly::Memory>()?
        .buffer();

    let location = v.as_ptr() as u32 / 4;

    Ok(js_sys::Float32Array::new(&memory_buffer).subarray(location, location + v.len() as u32))
}
