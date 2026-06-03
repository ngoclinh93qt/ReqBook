"use strict";

if (process.env.RQB_DOWNLOAD_ON_INSTALL !== "1" && process.env.MAD_DOWNLOAD_ON_INSTALL !== "1") {
  process.exit(0);
}

require("./rqb.js");
