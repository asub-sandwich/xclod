# xclod

A small CLI wrapper to aid in the process of calculating bulk density from turntable photogrammetry.

UNDER CONSTRUCTION 

# bd-utils DEPRICATED - IN: OLD_SCRIPTS 

A small CLI wrapper to aid in the process of calculating bulk density from turntable photogrammetry. 

+ `mov2jpg` - Extract JPEG frames from Apple `.mov` videos using ffmpeg
+ `heic2jpg` - Convert HEIC photos (files or directories) to JPEG using ImageMagick
+ `fbx2obj` - Convert Autodesk FBX models (files or directories) to OBJ using `assimp`

This script was designed to:
1. Work with both `magick convert` and `convert` depending on versioning
2. Optionally - use `parallel` to speed up directory conversions
3. Optionally - use `exiftool` to copy metadata from video frames

---

## Installation

Download script somewhere to your `PATH`, for example:
```bash
  cp bdutil ~/.local/bin
  # OR
  cp bdpy ~/.local.bin
  
  chmod +x "$HOME/.local/bin/bdutil"
  # OR
  chmod +x "$HOME/.local/bin/bdpy"
```

---

## Dependencies

**Required**
+ Python Script:
  + `mov2jpg`: `opencv-python` OR `ffmpeg`
  + `heic2jpg`: (`Pillow`, `pillow_heif`) OR ImageMagick (`magick` or `convert`, depending)
  + `fbx2obj`: `pyassimp` OR `assimp`

+ Bash Script:
  + `mov2jpg`: `ffmpeg`
  + `heic2jpg`: ImageMagick (`magick` or `convert`, depending)
  + `fbx2obj`: `assimp`

**Optional**
+ Bash Script:
  + `exiftool`
    + Used by `mov2jpg` to copy metadata from the source `.mov` to each extracted `.jpg`.
    + If not installed, only metadata found by `ffmpeg` will be transferred
  + `parallel`
    + Used by `heic2jpg` and `fbx2obj` to speed up directory conversions.
    + If missing, conversion will run in a sequential loop.
    + The script automatically sets a parallel job to use half of the available CPU cores.

---

## Notes

The script supports both:
+ `magick convert input.heic output.jpg` for ImageMagick 7
+ `convert input.heic output.jpg` for ImageMagick 6
+ Python tool is still undergoing testing, especially for metadata capture from `.mov` to `.jpg`


---

## License

GNU GPLv3.0
