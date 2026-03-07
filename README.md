# ttk4145_elevatorlab
Project in Sanntidssystemer by August Lind, Espen Johnsen Bentdal and Oliver Wahlen. 

If for sim "Unable to bind socket: Address already in use", use "sudo lsof -iTCP:15657 -sTCP:LISTEN" to find PID and then "sudo kill $PID". 