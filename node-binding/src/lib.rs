use chill_json::FuzzyJsonParser;
use neon::prelude::*;
use serde_json::Value;

fn to_js_value<'a>(cx: &'a mut FunctionContext, value: &Value) -> Handle<'a, JsValue> {
    match value {
        Value::Null => {
            let val = cx.null().upcast();
            val
        }
        Value::Bool(b) => {
            let val = cx.boolean(*b).upcast();

            val
        }
        Value::Number(n) => {
            let val = if let Some(i) = n.as_i64() {
                cx.number(i as f64).upcast()
            } else if let Some(u) = n.as_u64() {
                cx.number(u as f64).upcast()
            } else if let Some(f) = n.as_f64() {
                cx.number(f).upcast()
            } else {
                cx.null().upcast()
            };
            val
        }
        Value::String(s) => {
            let val = cx.string(s).upcast();
            val
        }
        Value::Array(arr) => {
            let js_array: Handle<'a, JsArray> = cx.empty_array();
            let mut vec = vec![];
            for (i, elem) in arr.iter().enumerate() {
                {
                    let js_value = to_js_value(cx, elem);

                    vec.push(js_value);
                    // let mut prop_opt = js_array.prop(cx, i as u32);
                    // prop_opt.set(js_value).unwrap();
                }
            }
            js_array.upcast()
        }
        Value::Object(map) => {
            let js_object = cx.empty_object();
            for (k, v) in map.iter() {
                let js_val = to_js_value(cx, v);
                js_object.set(cx, k.as_str(), js_val).unwrap();
            }
            js_object.upcast()
        }
    }
}

fn hello(mut cx: FunctionContext) -> JsResult<JsValue> {
    let js_string = cx.argument::<JsString>(0)?;
    let rust_string = js_string.value(&mut cx);
    let parser = FuzzyJsonParser::new();
    let result: serde_json::Value = parser.parse(&rust_string).unwrap();

    // Ok(cx.string("hello from rust"))
    // Convert serde_json::Value to JsValue, then return
    Ok(to_js_value(&mut cx, &result))
}

#[neon::main]
fn main(mut cx: ModuleContext) -> NeonResult<()> {
    cx.export_function("hello", hello)?;
    Ok(())
}
