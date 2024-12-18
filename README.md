# QOS

## Architecture
```
              ┌┐                                ┌┐                      
              ││                                ││                      
           process             QOS           process                    
           boundary                          boundary                   
              ││                                ││                      
┌──────┐      ││                                ││                      
│TPUFWD├─spsc─┼┼┐  ┌────────────────────────┐   ││                      
└──────┘      ││└──┼►┌─────┐  ┌──────────┐  │   ││   ┌─────────┐        
 ┌───┐        ││   │ │score├─►│ priority ├──┼───│├──►│sigverify│        
 │TPU├───spsc─┼┼───┼►└┬────┘  │  queue   │  │   ││   └─┬──┬────┘        
 └───┘        ││   │  │  ▲    └──────────┘  │   ││     │  │             
              ││   │  │  │    ┌────────────┐│   ││     │  │             
              ││   │  ▼  └────┤update model││ ┌─┼┼─spsc┘  ▼             
              ││   │ ┌───────┐└─────▲──────┘│ │ ││   ┌─────────┐
              ││   │ │partial│   ┌──┼──┐    │ │ ││   │scheduler├──etc──►
              ││   │ │  meta ├──►│meta │◄─fail┘ ││   └───┬─────┘        
              ││   │ │ store │   │store│    │   ││       │              
              ││   │ └───────┘   └──┬──┘◄─included───spsc┘              
              ││   └────────────────┼───────┘   ││                      
              ││                    │           ││                      
              ││                    │db         ││                      
              └┘                    ▼           └┘                      
```
The QoS sidecar is a single-threaded application that multiplexes over 4-6 input channels from different sources in a busy loop and processes the incoming messages respectively.
1. `TPU` & `TPUFWD`: Ingests incoming packets containing transactions and scores them with the latest version of the QoS Model. Transactions are placed in a fixed-capacity priority queue based on this score, and batches of the transactions with the highest scores are sent over to the sigverify stage. At this stage, a partial metadata entry is created for the transaction, which is missing the information about whether the transaction was included and the execution time (if included).  If a co-located relayer is being run on the same machine, there are two additional `RE1` and `RE2` channels corresponding to the TPU/FWD of the relayer.
2. `SIGVERIFY`: A feedback mechanism is included for transactions that fail sigverify. These feedback messages significantly reduce the score of the source IP. Future transactions from this IP are consequently assigned lower scores and are less likely to make it to the sigverify stage. Note that this feedback only mitigates the bad signature verify DOS vector and does not affect the signer score.
3. `SCHEDULER`: Information regarding whether a transaction is included and executed (regardless of whether the block successfully propagates and finalizes) is propagated back. These message are combined with the previously constructed partial entries to form a full metadata entry, which is then used to update the model and optionally sent to a database.


## Running the sidecar

After properly installing the sidecar into a process, such as the `agave-validator`, which involves adding a `que` producer for 
1. TPU & FWD
2. Failed sigverify signals
3. Completed scheduled signals
4. (Optional) recently confirmed signatures
   
as well as a consumer for the output of the sidecar [^1], follow these instructions.

### 0. Preallocate Huge Pages

If using huge pages, you must pre-allocate them before initializing all IPC channels and using the QoS sidecar.

### 1. Initialize IPC channels

Running

```
RUSTFLAGS="-C target-cpu=native" cargo build --release
```

will build all binaries. Once built, you can initialize IPC channels via

```
sudo ./target/release/solana-qos-cli
```

This is only necessary if using huge pages. Otherwise, the sidecar will use `/dev/shmem/`.

### 2. Run

After picking a random u64 seed, e.g. `420`, and a target packets-per-second (pps), e.g. `500` run qos via

```
sudo ./target/release/qos --xxhash-seed 420 --target-pps 500 --use-huge-pages
```

Note that `sudo` privileges are dropped after joining the IPC channels. 

[^1]: Temporal will release a patch for the agave validator that installs these IPC channels.
