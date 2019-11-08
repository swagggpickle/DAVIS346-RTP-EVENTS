package main

import (
	"encoding/csv"
	"fmt"
	"io"
	"log"
	"os"
)

func genFileReader(csvFileName string) *csv.Reader {
	csvFile, err := os.Open(csvFileName)
	if err != nil {
		log.Fatalln("Couldn't open the csv file", err)
	}
	r := csv.NewReader(csvFile)
	return r
}
func readLine(r *csv.Reader) []string {
	line, err := r.Read()
	if err == io.EOF {
		println("End of file")
		return []string{""}
	}
	if err != nil {
		log.Fatal(err)
	}
	return line
}
func genAvi(csvFileName string, width int32, degree int32, rate int32, numOfThread int32) {
	frameInterval := 1e6 / (rate)
	setDecExpLookup(frameInterval)
	setHSVColorLookup()

	// double the max number of frames per second
	// attempt to not make main collecting thread have to block
	// channels are basically blocking queue if no room is available
	framePool := make(chan *FullFrame, rate*3) // memory pool minimizing allocation at runtime
	frameQueue := make(chan *FullFrame, rate*3)
	writeQueue := make(chan *HSVColor, rate*3*numOfThread)
	quiteQueue := make(chan string, rate*2)

	/*
	* If we run out of frames there is a larger problem going on
	* we are no longer keeping up with live look at the timing of
	* frameWriteThread should be on average less than 16ms if not
	* we are not keeping up with the write
	* if it is within 16ms that means the data acquisition might be
	* getting flooded need to make that process more lean by increasing the pool
	 */
	for i := 0; i < cap(framePool); i++ { // so there are not dynamic allocations all the time

		framePool <- new(FullFrame)
	}
	// single copy that will be copied into one of the pool's instances
	singleFrame := FullFrame{frameInterval: frameInterval}

	// making threads if main is getting slowed down because the frameQueue
	// is blocking try increasing the number of thread
	for i := int32(0); i < numOfThread; i += 1 {
		go frameColorThread(frameQueue, framePool, writeQueue, quiteQueue)
	}
	go frameWriteThread("large.avi", frameQueue, framePool, writeQueue)
	// done spawning thread

	// opening file
	r := genFileReader(csvFileName)

	line1 := readLine(r) // read first line
	if line1 == nil {
		log.Panic("File was empty")
	}

	// only reading line two for init timestamp
	line2 := readLine(r)
	event := NewPixelEvent(line2)
	nextFrame := int32(event.timeStamp) + frameInterval

	var p float64 = 0.0
	frameCount := 0
	for {
		line2 = readLine(r)
		if len(line2) <= 3 {
			break
		}
		event.update(line2)
		if event.polarity == 1 {
			p = 1.0
		} else if event.polarity == 0 {
			p = -1.0
		}

		if event.timeStamp >= int(nextFrame) {
			// update private struct
			singleFrame.frameCount = int32(frameCount)
			singleFrame.nextFrame = nextFrame

			// ask pool for new struct ref blocks if none is available
			var tempFrame *FullFrame = <-framePool

			// copy struct
			*tempFrame = singleFrame // deep copy

			// add to output channel
			frameQueue <- tempFrame

			// update our private data
			frameCount += 1
			nextFrame += frameInterval
		}

		if p > 0.0 { // if 1
			singleFrame.arr[event.yAddress][event.xAddress] = 500
			singleFrame.timeArray[event.yAddress][event.xAddress] = float64(event.timeStamp)
		}

	}
	hold := <-quiteQueue
	fmt.Println("hold {}", hold)
	fmt.Println("DONE READING CSV")

}

func main() {
	println("Started up the program...")
	genAvi("large.csv", frameWidth, 5, frameRate, 1)
}
