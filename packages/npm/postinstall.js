"use strict";

if (process.env.TRELLIS_DOWNLOAD_ON_INSTALL !== "1") {
  process.exit(0);
}

require("./trellis.js");
