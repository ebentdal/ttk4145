hvorfor bruker vi rust?
 - memory safe 
 - raskt
 - support for async
 - good errorhandling, nice when things go wrong
 - no garbage collector
 - fearless concurrency

Thinking about elevators

The main problem of the project is to ensure that no orders are lost.
    What sub-problems do you think this consists of?
        - how do you devide tasks between elevators
        - how can you ensure the list is correct
        - what happens if one elevator dies - rip
    What will you have to make in order to solve these problems?
        - hiarchy (master slave perhaps). If 
        - What happens if master is wrong
        - Ensure what happens if mr master dies
        - double check masters order

Maybe try thinking about the happy case of the system:
    If we push the button one place, how do we make (preferably only) one elevator start moving?
      - master slave perhaps

    Once an elevator arrives, how do we inform the others that it is safe to clear that order?
    - broadcaster det

Maybe try thinking about the worst-case behavior of the system:

    What if the software controlling one of the elevators suddenly crashes?
    What if it doesn't crash, but hangs?
    - assume it ass dead, reassigne all orders
    What if a message between machines is lost?
    - 
    What if the network cable is suddenly disconnected? Then re-connected?
    - waits, restarts
    What if the elevator car never arrives at its destination?

arc btw, shared pointers

rust compiler tels you when you have something wrong, compile errors rather than run time error
