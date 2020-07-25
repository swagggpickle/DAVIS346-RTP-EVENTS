use csv;
use crate::dvs_const;


#[derive(Debug,Default)]
pub struct PixelFactory {
    timestamp_pos :usize,
	x_address_pos :usize,
	y_address_pos :usize,
	polarity_pos  :usize,

}
impl From<csv::StringRecord> for PixelFactory {
    fn from(record: csv::StringRecord) -> Self {
        let mut pe= PixelFactory::default();
        let mut i:usize = 0;
        for elem in record.iter() {
            match elem {
                "timeStamp" => pe.timestamp_pos = i,
                "xAddr" => pe.x_address_pos = i,
                "yAddr" => pe.y_address_pos = i,
                "polarity(0=OFF 1=ON)" => pe.polarity_pos = i,
                _ => println!("Unknown field {}", elem),
            }
            i = i + 1;
        }
        return pe;
    }
}
impl PixelFactory {
    pub fn make_pixel_event(&self, record: &csv::StringRecord) -> PixelEvent {
        PixelEvent {
            timestamp: record[self.timestamp_pos].parse().unwrap(), 
            polarity: record[self.polarity_pos].parse().unwrap(),
            x_address: dvs_const::DVS_X - record[self.x_address_pos].parse::<i32>().unwrap() - 1,
            y_address: dvs_const::DVS_Y - record[self.y_address_pos].parse::<i32>().unwrap() - 1
        }
    }
}
#[derive(Debug,Default)]
pub struct PixelEvent {
    pub timestamp :i32,
	pub x_address :i32,
	pub y_address :i32,
	pub polarity  :i32,
}

