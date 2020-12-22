# Todo

## Important

* Calibration
    * Do the apriltag calibration stuff when you hit enter
    * Write fragment shader to rotate images according to calibration results
    * You should be able to do the calibration in multiple stages
    * Pitch calibration math

* User interface to allow you to shuffle around images
* Handle errors (especially in orbit_station) properly
* Figure out how to check how many SBC's are plugged into the switch
* Allow orbit_helper to run on startup
    * Put it in the crontab or something
    * Also make sure it doesn't crash when we construct the TcpListener
    

## Low priority

* Add all of the open source licenses or at least figure out how copyright works
* Video recording mode for helper computers - save frames directly to disk

* Why doesn't 1080p streaming work with the modified v4l driver?

* Improve performance of the streaming mode
    * Use UDP instead of TCP??
    
* Make it easy to interface with this code for other purposes
    * Example Projects:
        * Streaming over the actual internet
        * Making 3d models of people
    * Rework architecture so that most of the important logic is in library crates


