"use strict";

import { request_episodes, request_shows } from "./http.js";

function render_shows(shows) {
  let ret = "<h1>Shows</h1>";

  for (const show_id in shows) {
    ret += '<a href="/show.html?show_id=' + show_id + '">';
    ret += shows[show_id].name;
    ret += "</a>";
    ret += "<br>";
  }
  return ret;
}

async function init() {
  const shows = await request_shows();

  const shows_list = document.getElementById("shows_list");
  shows_list.innerHTML = render_shows(shows);
}

init();
