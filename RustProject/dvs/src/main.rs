#![warn(unused_extern_crates)]
use csv;
use csv::Error;
use std::fs;
mod pixel_event;
mod dvs_const;
mod frame;
mod hsv_frame;
use clap::{Arg, App};
use opencv::prelude::*;
use std::sync::mpsc::{Sender, Receiver};
use std::sync::mpsc::channel;
use std::thread;
use threadpool::ThreadPool;
use std::collections::HashMap;
use std::time::{Instant};
use num_cpus;
fn main() -> Result<(), Error> {
    let matches = arguments();
    let filename = matches.value_of("file").unwrap();
    let decay_rate : f64 = matches.value_of("decay_rate").unwrap().parse().unwrap();
    if decay_rate <= 0.0 {
        panic!("Invalid decay rate. Must be > 0.0");
    }
    let frame_rate:i32 = matches.value_of("framerate").unwrap().parse().unwrap();
    if frame_rate <= 0 || frame_rate > 120 {
        panic!("Invalid frame rate size rate. Must be  0 < interlace < 121.");
    }
    let median_blur:i32 = matches.value_of("medianblur").unwrap().parse().unwrap();
    if median_blur < 1 || median_blur > 13  || median_blur % 2 == 0 {
        panic!("Invalid median blur. Must be  0 < interlace < 14 and odd.");
    }
    let output_file:String = matches.value_of("output_file").unwrap().parse().unwrap();
    if output_file.len() < 5 {
        panic!("Invlaid file name {}", output_file);
    }

    let color_range: hsv_frame::ColorRange = hsv_frame::ColorRange::new();
    
    println!("File Name:      {}", filename);
    println!("Decay Rate:     {}", decay_rate);
    println!("Frame Rate:     {}", frame_rate);
    println!("Median Blur:    {}", median_blur);


    let contents = fs::read_to_string(filename).expect("Something went wrong reading the file");
    let mut reader = csv::Reader::from_reader(contents.as_bytes());
    let headers= reader.headers().unwrap();
    let factory = pixel_event::PixelFactory::from(headers.clone());

    // let initial_pe = factory.make_pixel_event(reader.records().nth(0).expect("Empty csv").unwrap());
    let frame_interval = (1e6 / (frame_rate as f64)) as i32;
    let frame_rate = frame_rate as f64;
    let mut next_frame = frame_interval;
    let mut frame_count = 0;
    let mut frame = frame::Frame::new();
    frame.frame_interval = frame_interval;

    
    let decay_values = hsv_frame::DecayValues::new(frame_interval, decay_rate);
    let (tx, rx): (Sender<(hsv_frame::HSVColor, i32)>, Receiver<(hsv_frame::HSVColor, i32)>) = channel();
    let handle = thread::spawn(move || {
        frame_write(output_file, frame_rate, median_blur, rx);
    });
    let now = Instant::now();
    let mut current_frame = 0;
    for record in reader.records() {
        let record = record?;
        let pe = factory.make_pixel_event(record);
        if pe.timestamp > next_frame {
            frame.frame_count = frame_count;
            frame.next_frame = next_frame;
            
            let color_frame = hsv_frame::HSVColor::make_color(&frame, &color_range, &decay_values);
            tx.send((color_frame, current_frame)).unwrap();

            current_frame += 1;
            frame_count += 1;
            next_frame = pe.timestamp + frame_interval;

        }

        if pe.polarity == 1 {
            frame.arr[pe.x_address as usize][pe.y_address as usize] = 500 as f64;
            frame.time_array[pe.x_address as usize][pe.y_address as usize] = pe.timestamp as f64;
        } 
    }
    let new_now = Instant::now();
    let read_file_duration = new_now.checked_duration_since(now);
    drop(tx);
    handle.join().unwrap();
    let new_now = Instant::now();
    let file_completion = new_now.checked_duration_since(now);
    println!("Time to read time:  {:?}", read_file_duration);
    println!("Time to write file: {:?}", file_completion);
    Ok(())
}

fn file_writer(rx: Receiver<(opencv::core::Mat, i32)>, file_name: String, frame_rate: f64) {
    let frame_width       = 600;
    let height = frame_width * dvs_const::DVS_Y / dvs_const::DVS_X;
    let file_name = file_name + ".avi";
    let fourcc = opencv::videoio::VideoWriter::fourcc('M' as i8,'J' as i8,'P' as i8,'G' as i8).expect("How can this happen?");
    let writer = opencv::videoio::VideoWriter::new(&file_name[..], fourcc, frame_rate, opencv::core::Size::new(frame_width, height), true);
    let mut writer = match writer {
        Ok(writer) => writer,
        Err(e) => panic!(e),
    };
    let mut current_frame_count = 0;
    let mut map_key: HashMap<i32, opencv::core::Mat> = HashMap::new();
    loop {
        let val = rx.recv();
        let (val, frame_num) = match val {
            Ok(val) => val,
            Err(e) => {
                println!("{}", e);
                return;
            }
        };
        if frame_num != current_frame_count {
            map_key.insert(frame_num, val);
            continue;
        }
        current_frame_count += 1;
        writer.write(&val).unwrap();
        while map_key.contains_key(&current_frame_count) {
            match map_key.remove(&current_frame_count) {
                Some(val) => writer.write(&val).unwrap(),
                None => panic!("map did not have entry?"),
            }
            current_frame_count += 1;
        }

    }
}
fn frame_write(file_name: String, frame_rate: f64, blur_size: i32, rx: Receiver<(hsv_frame::HSVColor, i32)>) {
    let frame_width       = 600;
    let height = frame_width * dvs_const::DVS_Y / dvs_const::DVS_X;

    let (tx, final_rx): (Sender<(opencv::core::Mat, i32)>, Receiver<(opencv::core::Mat, i32)>) = channel();
    let file_writer = thread::spawn(move || {
        file_writer(final_rx, file_name, frame_rate);
    });
    let num_of_cpus = num_cpus::get();
    let pool = ThreadPool::new(num_of_cpus * 2);
    loop {
        let val = rx.recv();
        let (val, current_frame) = match val {
            Ok(val) => val,
            Err(e) => {
                println!("{}", e);
                break;
            }
        };

        let mat = val.arr;
        let sender = tx.clone();
        pool.execute(move || {
            process_frame(mat, sender, height, frame_width, blur_size, current_frame);
        }); 
    }
    drop(tx);
    file_writer.join().unwrap();
}

fn process_frame(mat: opencv::core::Mat, tx: Sender<(opencv::core::Mat, i32)>, height: i32, frame_width: i32, blur_size: i32, frame_num: i32) {
    let mut r_mat = unsafe {
        opencv::core::Mat:: new_rows_cols(height, frame_width, opencv::core::CV_8UC3).unwrap()
    };
    let size = r_mat.size().unwrap();
    opencv::imgproc::resize(&mat, &mut r_mat, size, 0.0, 0.0, opencv::imgproc::INTER_LINEAR).unwrap();
    
    let mut mb_mat = unsafe {
        opencv::core::Mat:: new_rows_cols( height, frame_width, opencv::core::CV_8UC3).unwrap()
    };
    opencv::imgproc::median_blur(&r_mat, &mut mb_mat, blur_size).unwrap();
    tx.send((mb_mat, frame_num)).unwrap();
}

fn arguments() -> clap::ArgMatches {
    App::new("My dvs app")
                    .version("0.0.0")
                    .author("Swaggg P. <swagggpickle@gmail.com>")
                    .about("Goal is to receive a steam of pixels from the dvs David346 \ncamera and display a decay on pixels that have not changed \nover time")
                    .arg(Arg::with_name("file")
                            .long("file")
                            .value_name("Input File")
                            .about("File to read from to construct video required to be csv.")
                            .default_value("large.csv")
                            .takes_value(true))
                    .arg(Arg::with_name("decay_rate")
                            .long("decay_rate")
                            .default_value("0.15")
                            .about("Determinds the rate in which a pixel will decay.")
                            .takes_value(true))
                    .arg(Arg::with_name("framerate")
                            .long("framerate")
                            .default_value("60")
                            .about("Frame rate.")
                            .takes_value(true))
                    .arg(Arg::with_name("medianblur")
                            .long("medianblur")
                            .default_value("5")
                            .about("Median blur")
                            .takes_value(true))
                    .arg(Arg::with_name("output_file")
                            .long("output_file")
                            .default_value("result")
                            .about("Name of the file containing the vidoe.")
                            .takes_value(true)).get_matches()        
}