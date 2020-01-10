# NDI Router
This is a work in progress to create an NDI router. The basic idea is a fixed amount of outputs is created that can be used by NDI receivers.

This is written in in rust so the following commands can be used to compile and run. It will require the NDI libs to be somewhere on the PATH.

```bash
cargo build
cargo run
```

## TODO
- [x] Fetch NDI sources on network
- [ ] Create TCP server to control
- [ ] Using NDI routing API to foreward sources
- [ ] Make compatible with BMD videohub spec