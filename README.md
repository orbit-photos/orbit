# Proprietary software for the station computer

# Setup

## Network config

Run the command `nm-connection-editor`, select the ethernet option,
then go to the IPv4 Settings tab. For the 'Method' dropdown, select 'Shared to other computers'.
Then, Add a new row to the 'Address' table, with Address=192.168.2.1, Netmask=24, and Gateway blank.

## Necessery packages:

* Packages needed for building ffmpeg-sys-next Rust library including:
  ffmpeg clang libavcodec-dev libavdevice-dev libavfilter-dev libavformat-dev
  libavresample-dev libavutil-dev libpostproc-dev libswresample-dev libswscale-dev pkg-config
* apriltag (https://github.com/AprilRobotics/apriltag)
* others I'm forgetting

----------------------------------------------------------------------------------

# Proprietary software for the single-board computers

# Setup

## Device configuration
Model in use is NanoPi NEO-LTS (https://www.friendlyarm.com/index.php?route=product/product&product_id=132).
The operating system currently in use is 'Armbian 20.08.5 Buster with Linux 5.8.11-sunxi'.
We might need to reformat the disk after we put the image on?? I used the disks program on my computer.
We have to add a significant amount of swap so the system doesn't crash. Also, make sure to keep the
computers cool (with fans and heatsinks).

Make the hostname helper0, helper1, helper2, etc. The username should be identical to the hostname. The
password should be set as 'armbian'

### Network Config 

* Run `nmtui`
* Select Edit a Connection
* Select Add
* Make the type Ethernet
* Make the profile name Static
* Make the device `eth0`
* Click Show under IPv4 to expand that section
* Add 192.168.2.10x/24 to the Addresses section
* Make the gateway 192.168.2.1
* Add 192.168.2.1 to the DNS servers
* Delete the other connections

Packages that must be installed on the helper computers include:
* v4l-utils (maybe just for debugging, but it might even be necessary)
* libv4l-dev (probably)
* other stuff I'm sure

Then you must follow the instructions in orbit-photos/uvc-driver-fixed-bandwidth for
permanently applying the bandwidth fix

The `orbit_helper` binary should be installed at `/home/helper_/orbit_helper`

We probably want to make orbit_helper run automatically
in rc.local

## Building `orbit_helper`:

We're gonna want to cross compile `orbit_helper` to target `armv7-unknown-linux-gnueabihf`.
In order to do that, we need these packages on the computer that's gonna be doing the compiling:
* https://git.linuxtv.org/v4l-utils.git/
  For help building that, there's an article https://git.linuxtv.org/v4l-utils.git/tree/INSTALL
  Look at the section called 'Cross Compiling:', subsection about the Linaro toolchain
  We first install our Linaro toolchain to `/opt/gcc-linaro-7.5.0-2019.12-x86_64_arm-linux-gnueabihf`
  Then we will build with the commands:
```shell
export PATH=/opt/gcc-linaro-7.5.0-2019.12-x86_64_arm-linux-gnueabihf/bin:$PATH
export PKG_CONFIG_LIBDIR=/opt/gcc-linaro-7.5.0-2019.12-x86_64_arm-linux-gnueabihf/arm-linux-gnueabihf/
./configure --host=arm-linux-gnueabihf --without-jpeg
make
```
You're gonna get weirdo errors and I'm not sure how to help you. I might have run `./bootstrap`?
You'll have to install lots of packages too. Not quite sure how I got it to work