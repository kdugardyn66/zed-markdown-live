# MarkDown Live — pictures, videos and audio (0.3.0)

Pictures use the normal Markdown syntax; size them with `{width=300}`. A picture
alone in its paragraph with a title (`"…"`) gets a caption. HEIC and TIFF are
converted for the browser (`sips` on macOS). Videos use the same syntax:
MKV/AVI/… are converted with ffmpeg in the background, then cached.

## Pictures

![png](media/pic.png) ![jpeg](media/pic.jpg){width=120} ![bmp](media/pic.bmp) ![svg](media/pic.svg)

![heic](media/pic.heic) ![tiff](media/pic.tiff)

![A captioned picture](media/pic.png "Figure 1: a caption from the title")

@import "media/pic.heic" {width=200 title="imported HEIC"}

## Videos

![mp4](media/clip.mp4)

![webm](media/clip.webm){width=240}

![mkv](media/clip.mkv)

![mov](media/clip.mov)

![avi](media/clip.avi)

@import "media/clip.mp4" {autoplay loop start=1}

## Audio and embeds

![tone](media/tone.mp3)

![A video on YouTube](https://youtu.be/aqz-KE-bpKQ)
