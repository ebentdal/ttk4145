Before proceeding with any project-related networking code, think about how you would solve these problems, and what you need in order to solve them.

Guarantees about elevators:
    What should happen if one of the nodes loses its network connection?
        - trying to reconnect, if not during some time, redistribute the tasks. Reassign all requests
    What should happen if one of the nodes loses power for a brief moment?
        - timeout() cap, same as connection

    What should happen if some unforeseen event causes the elevator to never reach its destination, but communication remains intact?

Guarantees about orders:
    Do all your nodes need to "agree" on a call for it to be accepted? In that case, how is a faulty node handled?
    How can you be sure that a remote node "agrees" on an call?
    How do you handle losing packets between the nodes?
    Do you share the entire state of the current calls, or just the changes as they occur?
        For either one: What should happen when an elevator re-joins after having been offline?

Pencil and paper is encouraged! Drawing a diagram/graph of the message pathways between nodes (elevators) will aid in visualizing complexity. Drawing the order of messages through time will let you more easily see what happens when communication fails.

Topology:
    What kind of network topology do you want to implement? Peer to peer? Master slave? Circle? Something else?
    In the case of a master-slave configuration: Do you have only one program, or two (a "master" executable and a "slave")?
        How do you handle a master node disconnecting?
        Is a slave becoming a master a part of the network module?
    In the case of a peer-to-peer configuration:
        Who decides the order assignment?
        What happens if someone presses the same button on two panels at once? Is this even a problem?
    Remember that you only have three machines available, no outside always-online fourth machine is permitted.

Technical implementation and module boundary:
    Protocols: TCP, UDP, or something else?
        If you are using TCP: How do you know who connects to who?
            Do you need an initialization phase to set up all the connections?
        If you are using UDP broadcast: How do you differentiate between messages from different nodes?
        If you are using a library or language feature to do the heavy lifting - what is it, and does it satisfy your needs?
    Do you want to build the necessary reliability into the module, or handle that at a higher level?
    Is detection (and handling) of things like lost messages or lost nodes a part of the network module?
    How will you pack and unpack (serialize) data?
        Do you use structs, classes, tuples, lists, ...?
        JSON, XML, plain strings, or just plain memcpy?
        Is serialization a part of the network module?
