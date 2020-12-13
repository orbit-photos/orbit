# Proprietary Software for Orbit Photo Booth

# Setup

## Network config  

Run the command `nm-connection-editor`, select the ethernet option, 
then go to the IPv4 Settings tab. For the 'Method' dropdown, select 'Shared to other computers'.
Then, Add a new row to the 'Address' table, with Address=192.168.2.1, Netmask=24, and Gateway blank.

## Necessery packages:

* Packages needed for building ffmpeg-sys-next Rust library like:
ffmpeg clang libavcodec-dev libavdevice-dev libavfilter-dev libavformat-dev 
libavresample-dev libavutil-dev libpostproc-dev libswresample-dev libswscale-dev pkg-config 

* apriltag (https://github.com/AprilRobotics/apriltag)
* others I'm forgetting


