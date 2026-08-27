# tcpp03-m20

A `no_std` async driver for the [TCPP03-M20](https://www.st.com/en/interfaces-and-transceivers/tcpp03-m20.html) USB-C power delivery gate controller, built on `embedded-hal`/`embedded-hal-async`.

## Example

```rust
use tcpp03_m20::{Device, PdRole, CcState};

let mut device = Device::new(delay, i2c, enable_pin, flgn_pin, /* vddio */ false, PdRole::Sink);

device.init().await?;
// ... detects attach with PD comms. on CC1
device.attach(CcState::CC1).await?;
// ... later, on disconnect ...
device.detach().await?;
```

## Features

- `defmt` — enable `defmt::Format` implementations for logging.
