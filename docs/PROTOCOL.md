# MINI_KEYBOARD USB protocol (VID 1189 / 514C, PID 88xx)

This is the configuration protocol of the cheap programmable USB macro pads that ship with the
**MINI_KEYBOARD** software (Windows `MINI_KEYBOARD.exe`, macOS `MINI_KEYBOARD.app`). It was
reconstructed by disassembling the vendor's macOS application (Qt 5 + hidapi 0.12) and checked
against packets captured from real devices by
[macropad_tool](https://github.com/kamaaina/macropad_tool).

Vendor function names from the binary are quoted (e.g. `Widget::HID_write`) so the findings can be
verified.

## Devices

The vendor software (`Widget::timeoutSlot`) probes these IDs, in this order:

| Vendor ID | Product IDs |
|---|---|
| `0x1189` | `0x8842`, `0x8840`, `0x8830`, `0x8831`, `0x8832`, `0x8833`, `0x8850`, `0x8851` |
| `0x514C` | same list |

It opens the HID interface whose `interface_number == 0`. On Linux the configuration channel is the
hidraw node whose report descriptor declares report ID 3 (usually a vendor-defined usage page).

The older `0x8890` product uses a different protocol and is not covered here.

## Transport

Every command is an HID **output report with report ID `0x03`**, 65 bytes including the ID
(`hid_write(dev, buf, 0x41)`). Replies are 64-byte input reports, also starting with `0x03`.

| Command | Bytes | Notes |
|---|---|---|
| Identify | `03 FB FB FB` | reply: `[2]` = keys, `[3]` = knobs, `[4]` = drawing variant |
| Read layer | `03 FA <keys> <knobs> <layer>` | one reply per control, same layout as a key record |
| Program key | `03 FD <id> <layer> <mode> <delay lo> <delay hi> 00 00 00 <n> (<mod> <code>)…` | layers are 1–3 |
| LED | `03 FE B0 <layer> 08 00 00 00 00 00 01 00 <color<<4 \| mode>` | per layer |
| Store layer | `03 FD FE FF` | sent after the records of each layer; the vendor app then sleeps 1 s |
| Firmware update | `03 EF` | enters the bootloader; **not implemented on purpose** |

`Widget::HID_write` only sends the records of controls that changed, then the store command for
every layer that had changes. A cleared key is sent as mode 1 with `n = 0`.

## Key record

Offsets are on the wire (byte 0 is the report ID).

| Byte | Meaning |
|---|---|
| 1 | `0xFD` |
| 2 | control id (see below) |
| 3 | layer, 1–3 |
| 4 | mode: `1` keyboard, `2` media, `3` mouse, `5` keyboard with delay, `8` LED |
| 5–6 | delay between chords in ms, little endian (mode 5) |
| 10 | number of entries |
| 11… | entries |

- **Keyboard (mode 1/5)**: entries are `(modifiers, HID usage)` pairs, up to 17 are known to work.
  Modifier bits: `01` Ctrl, `02` Shift, `04` Alt, `08` Win, `10` RCtrl, `20` RShift, `40` AltGr,
  `80` RWin. A pair with usage `0` sends modifiers only. A delayed sequence is sent as a mode 1
  record followed by the same record with mode 5 and the delay.
- **Media (mode 2)**: `n = 2`, then the 16-bit consumer usage little endian
  (`Widget::SetMulKey`), e.g. volume up = `02 E9 00`.
- **Mouse (mode 3)**: `n = 1`, `[11]` modifier (`01` Ctrl, `02` Shift, `04` Alt), `[12]` buttons
  (`01` left, `02` right, `04` middle), `[15]` wheel (`01` up, `FF` down).
- **LED (mode 8)**: control id `0xB0`, `[10] = 1`, `[12]` = colour in the high nibble
  (1 red, 2 orange, 3 yellow, 4 green, 5 cyan, 6 blue, 7 purple) and mode in the low nibble
  (0 off, 1 steady colour, 2 flash on press, 3 flash variant, 4 light the pressed key,
  5 steady white).

## Control ids

The vendor UI has 24 controls with ids 1–24 (`QButtonGroup::addButton(button, id)`):

- keys: `1 … N`
- knobs: three ids each — rotate left, press, rotate right — starting at `0x10` (16)
- exceptions, because only 24 ids exist:
  - 12 keys + 4 knobs: knob 1 = 13/14/15, knobs 2–4 from 16
  - 21 keys + 1 knob: knob = 22/23/24
- "1 knob + 1 wheel" pads (`Widget::Set_Keyboard_0add1`): 16/17/18 plus 19 and 21 (a second
  knob without press)

## Model detection

`Widget::Identify_KeyBoard_style` maps the identify reply `(keys, knobs)` to a layout:

`0+1`, `0+2` (drawn as 0+1 unless PID is `0x8850`), `0+3`, `1`, `2`, `3`, `3+1`, `4`, `4+1`
(alternative artwork when `[4] = 1` or PID `0x8851`), `4+3`, `5`, `6`, `6+1`, `6+2`, `9+2`,
`9+3`, `11+3`, `12+2`, `12+3`, `12+4`, `16`, `21+1`. Any other combination is drawn as `15+3`.

The read request echoes the `keys` and `knobs` bytes reported by the device. Some `0x8840` units
never answer read requests; they can still be programmed.
