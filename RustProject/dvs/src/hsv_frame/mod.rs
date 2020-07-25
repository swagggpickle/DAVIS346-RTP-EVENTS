use crate::dvs_const;
use crate::frame;
use opencv::prelude::*;
#[derive(Default,Debug,Clone, Copy)]
pub struct Color {
    pub data: [u8; 3],
}
impl Color {
    pub fn new(data: [f64; 3]) -> Color {
        Color::hsv_2_rgb(data)
    }
    fn switcher(h: f64, a: f64, b: f64, c: f64, v:f64) -> [u8; 3] {
        let h = h as i64;
        let a = a as u8;
        let b = b as u8;
        let c = c as u8;
        let v = v as u8;
        match h {
            0 => [v, c, a],
            1 => [b, v, a],
            2 => [a, v, c],
            3 => [a, b, v],
            4 => [c, a, v],
            5 => [v, a, b],
            _ => [0, 0, 0],
        }
    }
    fn hsv_2_rgb(r: [f64; 3]) -> Color {
        let s = r[1] / 100.0; // either zero or one
        let v = r[2] / 100.0; // either zero or one
        let mut h = r[0] / 360.0;
    
        if s >= 0.0 {
            if h >= 1.0 {
                h = 0.0;
            }
            h = 6.0 * h;
            let f = h - ((h as i64) as f64);
            let a = 255.0 * v * (1.0 - s);
            let b = 255.0 * v * (1.0 - (s * f));
            let c = 255.0 * v * (1.0 - (v * (1.0 - f)));
            let v = 255.0 * v;
    
            return Color {
                data: Color::switcher((h as i64) as f64, a, b, c, v),
            };
        } else {
            let v = v as u8;
            return Color {
                data: [v * 255, v, v],
            }
        }
    }
}

#[derive(Default,Debug,Clone)]
pub struct ColorRange {
    colors: Vec<Color>,
}
impl ColorRange {
    pub fn new() -> Self {
        let mut color = Color{data: [0u8; 3],};
        let mut vec:Vec<Color> = Vec::new();
         for i in 0..=256*2{
            let i = i as f64 * 0.5;
            vec.push(color);
            color = Color::new([i, 100.0, 100.0]);
        }
        return ColorRange{
            colors: vec,
        };
    }
}
#[derive(Default,Debug,Clone)]
pub struct DecayValues {
    vals: Vec<f64>,
}
impl DecayValues {
    pub fn new(frame_interval: i32, decay_rate: f64) -> Self {
        let mut exp_val = 500.0;
        let inc = frame_interval as f64* 2.0;
        let mut iteration = 1.0;
        let mut r_val = DecayValues::default();
        while exp_val >= 1.0 {
            r_val.vals.push(exp_val);
            exp_val = 500.0 * (1.0-decay_rate).powf(iteration/frame_interval as f64);
            iteration += inc;
        }
        return r_val;
    }
}

pub struct HSVColor {
    pub frame_count :i32,
    pub arr : opencv::core::Mat,
}

impl HSVColor {
    fn color(&mut self,frame: &frame::Frame, range: &ColorRange, decay_values: &DecayValues) {
        let len_decay_values = decay_values.vals.len() as usize;
        let i_frame_interval = 1.0 / frame.frame_interval as f64;
        let next_frame = frame.next_frame as f64;
        
        for (i, row) in frame.time_array.iter().enumerate() {
            for (y, col) in row.iter().enumerate() {
                let elem = self.arr.at_2d_mut::<opencv::core::Vec3b>(y as i32, i as i32);
                let elem = match elem {
                    Ok(elem) => elem,
                    Err(e) => panic!("{}",e),
                };
                
                let index = ((next_frame - col) * i_frame_interval) as usize;
                let decay_dx_dy =  if index < len_decay_values {
                    decay_values.vals[index as usize] as usize
                } else {
                    0 as usize
                };
                *elem = opencv::core::Vec3::<u8>::from(range.colors[decay_dx_dy].data);  
            }
        }
    }
    pub fn make_color(frame: &frame::Frame, range: &ColorRange, decay_values: &DecayValues) -> Self {
        let mut color_frame= HSVColor{
            frame_count: frame.frame_count,
            arr: unsafe {
                    opencv::core::Mat::new_rows_cols(dvs_const::DVS_Y, dvs_const::DVS_X , opencv::core::CV_8UC3).unwrap()
            },
        };
        color_frame.color(frame, range, decay_values);
        return color_frame;
    }
}