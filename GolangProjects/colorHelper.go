package main

import (
	"fmt"
	"gocv.io/x/gocv"
	"image"
	"math"
	"os"
	"strconv"
	"sync"
	"time"
)
const (
	dvsX             = 346
	dvsY             = 260
)
var (
	fileName string
	decayRate        = .15
	interLaceSize    = 1
	frameRate        = 60
	frameWidth       = 600
	MedianBlurKSize  = 5
)

type ColorType struct {
	data [3]uint8
}

var decExpValues []float64
var hsvColorValues []ColorType

type PixelEvent struct {
	timeStamp int
	xAddress  int
	yAddress  int
	polarity  int
}

type FullFrame struct {
	frameCount    int32
	arr           [dvsY][dvsX]float64
	timeArray     [dvsY][dvsX]float64
	nextFrame     int32
	frameInterval int32
}

type HSVColor struct {
	frameCount int32
	arr        [dvsY][dvsX]*ColorType
}

func stringToInt(str string) int {
	i, err := strconv.Atoi(str)
	if err != nil {
		// handle error
		fmt.Println(err)
		os.Exit(2)
	}
	return i
}
func NewPixelEvent(lineSlice []string) *PixelEvent {
	pe := PixelEvent{polarity: stringToInt(lineSlice[3]),
		yAddress:  (dvsY - 1) - stringToInt(lineSlice[2]),
		xAddress:  (dvsX - 1) - stringToInt(lineSlice[1]),
		timeStamp: stringToInt(lineSlice[0])}
	return &pe
}
func (p *PixelEvent) update(lineSlice []string) {
	p.polarity = stringToInt(lineSlice[3])
	p.yAddress = (dvsY - 1) - stringToInt(lineSlice[2])
	p.xAddress = (dvsX - 1) - stringToInt(lineSlice[1])
	p.timeStamp = stringToInt(lineSlice[0])
}

func setDecExpLookup(frameInterval int32) {
	var expVal = 500.0
	var incVal = float64(frameInterval) * 2.0
	var iteration float64 = 1
	for expVal >= 1 {
		decExpValues = append(decExpValues, expVal)
		expVal = 500.0 * math.Pow(1-decayRate, iteration/float64(frameInterval))
		iteration += incVal
	}
}

func setHSVColorLookup() {
	var incVal = 0.5
	var iterColor = 1.0
	var colorArray = [3]uint8{0.0, 0.0, 0.0}
	for iterColor <= 256 {
		var tempColor = ColorType{data: colorArray}
		hsvColorValues = append(hsvColorValues, tempColor)
		colorArray = hsv2rgb([3]float64{iterColor, 100.0, 100.0}) // todo: modify this to the special algorithm that kass made
		iterColor += incVal
	}
}

func switcher(h, a, b, c, v float64) [3]uint8 {
	switch math.Floor(h) {
	case 0:
		return [3]uint8{uint8(v), uint8(c), uint8(a)}
	case 1:
		return [3]uint8{uint8(b), uint8(v), uint8(a)}
	case 2:
		return [3]uint8{uint8(a), uint8(v), uint8(c)}
	case 3:
		return [3]uint8{uint8(a), uint8(b), uint8(v)}
	case 4:
		return [3]uint8{uint8(c), uint8(a), uint8(v)}
	case 5:
		return [3]uint8{uint8(v), uint8(a), uint8(b)}
	default:
		return [3]uint8{0.0, 0.0, 0.0}
	}

}

func hsv2rgb(r [3]float64) [3]uint8 {
	var s = r[1] / 100.0 // either zero or one
	var v = r[2] / 100.0 // either zero or one
	var h = r[0] / 360.0

	if s >= 0.0 {
		if h >= 1 {
			h = 0
		}
		h = 6.0 * h
		f := h - math.Floor(h)
		a := math.Round(255 * v * (1.0 - s))
		b := math.Round(255 * v * (1.0 - (s * f)))
		c := math.Round(255 * v * (1.0 - (v * (1.0 - f))))
		v = math.Round(255 * v)

		return switcher(math.Floor(h), a, b, c, v)
	} else {
		return [3]uint8{uint8(math.Round(v * 255)), uint8(v), uint8(v)}
	}
}
func colorMap(frame *FullFrame) *HSVColor {
	hsvColorFrame := &HSVColor{}
	lengthOfDEV := int32(len(decExpValues))
	var iFrameInterval = 1.0 / float64(frame.frameInterval)
	var decayedDxDy int32 = 0
	nextFrame := float64(frame.nextFrame)
	for dY := 0; dY < dvsY; dY++ {
		for dX := 0; dX < dvsX; dX++ {
			index := int32((nextFrame - frame.timeArray[dY][dX]) * iFrameInterval)
			if index < lengthOfDEV {
				decayedDxDy = int32(decExpValues[index])
			} else {
				decayedDxDy = 0.0
			}

			if decayedDxDy >= 1 && decayedDxDy < int32(len(hsvColorValues)) {
				hsvColorFrame.arr[dY][dX] = &hsvColorValues[decayedDxDy]
			} else {
				hsvColorFrame.arr[dY][dX] = &hsvColorValues[0]
			}
		}
	}
	return hsvColorFrame
}

func frameColorThread(frameQueue, framePool chan *FullFrame, writeQueue chan *HSVColor, quiteChannel chan string) {
	for {
		//start := time.Now()
		var videoFrame *FullFrame = <-frameQueue
		hsv := colorMap(videoFrame)
		hsv.frameCount = videoFrame.frameCount
		framePool <- videoFrame // put back to mempool
		writeQueue <- hsv
		//fmt.Println("time since start ", time.Since(start))
		select {
		case <-quiteChannel:
			break
		default:
			continue
		}
	}
}
func setColorAt(x, y int, color *HSVColor, mat *gocv.Mat) {
	nChannels := mat.Channels()
	for i := 0; i < nChannels; i++ {
		mat.SetUCharAt(y, x*nChannels+i, color.arr[y][x].data[i])
	}
}

type TransformType struct {
	color    *HSVColor
	mat      *gocv.Mat
	startPos int
	y        int
	wg       *sync.WaitGroup
}

func threadedColorPool(tChan chan TransformType) {
	for {
		toUpdate := <-tChan
		for dX := toUpdate.startPos; dX < dvsX; dX += interLaceSize {
			setColorAt(dX, toUpdate.y, toUpdate.color, toUpdate.mat)
		}
		toUpdate.wg.Done()
	}
}

func hsvColorToMat(color *HSVColor, mat *gocv.Mat, startPos int, tChan chan TransformType) {
	var wg sync.WaitGroup // opposite of semaphore counts down and when zero .Wait() releases
	toTransform := TransformType{wg: &wg, y: 0, startPos: startPos, mat: mat, color: color}
	wg.Add(dvsY)
	for dY := 0; dY < dvsY; dY += 1 {
		// for each column let a worker handle all the X
		toTransform.y = dY
		tChan <- toTransform // add to pool to do work
	}
	wg.Wait() // wait until all the work is done so we can display
}
func writeLive(writeChan chan *gocv.Mat) {
	writer := gocv.NewWindow("Face Detect")
	for {
		write := <- writeChan
		writer.IMShow(*write)
		writer.WaitKey(16)

	}
}

func frameWriteThread(fileName string, frameQueue, framePool chan *FullFrame, writeQueue chan *HSVColor) {
	height := frameWidth * dvsY / dvsX

	writer, err := gocv.VideoWriterFile(fileName, "MJPG", float64(frameRate), frameWidth, height, true)
	if err != nil {
		fmt.Println("failed to open write file")
		os.Exit(0)
	}
	tChan := make(chan TransformType, dvsY)
	for i := 0; i < dvsY; i++ {
		go threadedColorPool(tChan)
	}
	var keyVal = make(map[int32]*HSVColor)
	var currentFrame int32 = 0

	s    := gocv.NewScalar(255.0, 255.0, 180.0, 0.0)
	mMat := gocv.NewMatWithSizeFromScalar(s, dvsY, dvsX, gocv.MatTypeCV8UC3)

	//defer window.Close()
	writeChan := make(chan *gocv.Mat, 60*3*4)
	go writeLive(writeChan)

	rowPos := 0
	for {
		toWrite := <-writeQueue // waits for frame color thread to produce results
		start := time.Now()     // for performance analysis

		if currentFrame == toWrite.frameCount {
			hsvColorToMat(toWrite, &mMat, rowPos, tChan)
			rowPos += 1
			rowPos %= interLaceSize
			rMat := gocv.NewMatWithSizeFromScalar(s, dvsY, dvsX, gocv.MatTypeCV8UC3)
			gocv.Resize(mMat, &rMat, image.Point{X: frameWidth, Y: height}, 0, 0, gocv.InterpolationLinear)
			gocv.MedianBlur(rMat, &rMat, MedianBlurKSize)
			go writer.Write(rMat)
			writeChan <- &rMat // live stream write
			currentFrame += 1
			for val, ok := keyVal[currentFrame]; ok; {
				hsvColorToMat(val, &mMat, rowPos, tChan)
				rowPos += 1
				rowPos %= interLaceSize
				rMat = gocv.NewMatWithSizeFromScalar(s, dvsY, dvsX, gocv.MatTypeCV8UC3)
				gocv.Resize(mMat, &rMat, image.Point{X: frameWidth, Y: height}, 0, 0, gocv.InterpolationLinear)
				gocv.MedianBlur(rMat, &rMat, MedianBlurKSize)
				go writer.Write(rMat)
				writeChan <- &rMat
				delete(keyVal, currentFrame)

				currentFrame += 1
				val, ok = keyVal[currentFrame]
			}
		} else {
			fmt.Println("going to write to keyVal map")
			keyVal[toWrite.frameCount] = toWrite
		}
		fmt.Println("time since write ", time.Since(start), " current frame ", currentFrame)

	}
}
