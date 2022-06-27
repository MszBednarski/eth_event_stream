# eth_event_stream

[![CI](https://github.com/MszBednarski/eth_event_stream/actions/workflows/main.yml/badge.svg)](https://github.com/MszBednarski/eth_event_stream/actions/workflows/main.yml)

*Synchronized multiple type event streaming for arbitrary time frames*

So you want to get events from the ethereum like blockchains. You wrote some event streaming, live and historical getters, but you want more. You tried writing some in typescript but you needed more threads, you wrote in python is also not it. Welcome you can rest here.

# You want
- To know more complex products of blockchain state over time? No problem just stream synchronized events and monitor the state changes.
- To build an exchange? No problem just stream synchronized events and monitor the state changes.
- To stream events into a timeseries database? No problem just stream synchronized events and monitor the state changes.

# Example
For instance I want all events from the block 15013349 till now + 10000. No problem. The API is the same you will just have to wait longer. No more getting in batches with the Alchemy limit of 2k blocks and even smaller limits for other providers. No more disjoint ways of handling historical events and events coming in live. Just use this, set the from_block and to_block and events will stream from separate threads and be published as synchronized blocks.

# API