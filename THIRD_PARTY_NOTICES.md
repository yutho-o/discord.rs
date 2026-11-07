# Third party notices

discord.rs is a Rust port of discord.js. Translating code into another language is treated as a
derivative work throughout, so these notices apply even to files with no line copied verbatim.

This is engineering guidance from reading the licence texts, not legal advice.

| Source | Licence | Copyright lines retained | Used for |
|---|---|---|---|
| `discord.js` (root and `packages/discord.js`) | Apache-2.0 | 2021 Noel Buechler, 2015 Amish Shah | Behaviour, structure, tests |
| `packages/ws` | Apache-2.0 | 2022 Noel Buechler, 2022 Denis Cristea | `dr_gateway` |
| `packages/voice` | Apache-2.0 | 2022 Noel Buechler, 2020 Amish Shah | `dr_voice` (later, not in the proof of concept) |
| `packages/builders` | Apache-2.0 | 2021 Noel Buechler, 2021 Vlad Frangu | `dr_builders` |
| `packages/collection` | Apache-2.0 | 2021 Noel Buechler, 2015 Amish Shah | `Collection` (later, not in the proof of concept) |
| `packages/formatters` | Apache-2.0 | 2021 Noel Buechler, 2021 Vlad Frangu | `fmt`, `escape` (later, not in the proof of concept) |
| `packages/util` | Apache-2.0 | 2022 Noel Buechler | `util` |
| `packages/core` | Apache-2.0 | 2022 Noel Buechler, 2022 Suneet Tipirneni | REST groups |
| `packages/rest` | Apache-2.0 | No copyright line in its `LICENSE` (re-checked at L0). Covered by the root lines | `dr_rest` |
| `discord-api-types` | MIT | (c) 2020 vladfrangu | `dr_model`, doc comments |
| `prism-media` 1.3.5 | Apache-2.0 | 2019 - 2020 Amish Shah | Demuxer behaviour (later, not in the proof of concept) |

No upstream package has a `NOTICE` file within depth 3 of its repository, so Apache-2.0 section 4(d)
has nothing to copy. Dependencies keep their own licences and are listed, not relicensed.

## discord-api-types, MIT License

MIT License

Copyright (c) 2020 vladfrangu

Permission is hereby granted, free of charge, to any person obtaining a copy
of this software and associated documentation files (the "Software"), to deal
in the Software without restriction, including without limitation the rights
to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
copies of the Software, and to permit persons to whom the Software is
furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in all
copies or substantial portions of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
SOFTWARE.
