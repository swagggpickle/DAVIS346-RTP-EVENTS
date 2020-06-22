use crate::dvs_const;

#[derive(Clone, Copy)]
pub struct Frame {
    pub frame_count    :i32,
	pub arr            :[[f64; dvs_const::DVS_Y as usize]; dvs_const::DVS_X as usize],
	pub time_array     :[[f64; dvs_const::DVS_Y as usize]; dvs_const::DVS_X as usize],
	pub next_frame     :i32,
	pub frame_interval :i32
}

impl Frame {
    pub fn new() -> Self {
        Frame {
            frame_count : 0,
            arr: [[0.0; dvs_const::DVS_Y as usize]; dvs_const::DVS_X as usize],
            time_array: [[0.0; dvs_const::DVS_Y as usize]; dvs_const::DVS_X as usize],
            next_frame: 0,
            frame_interval: 0
        }
    }
}