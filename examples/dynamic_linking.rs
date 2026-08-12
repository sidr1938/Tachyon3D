// cargo run -p tachyon3d --example dynamic_linking --features dynamic_linking
use tachyon3d::*;
use tachyon3d_internal::app::AppT3D;

fn main() {
    println!("Changed");
    let app = AppT3D::new();

}
