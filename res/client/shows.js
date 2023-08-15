"use strict";

import { get_shows } from "./http.js";

function sort_shows_by_name(shows) {
  shows.sort((a, b) => a[1].name.toLowerCase() > b[1].name.toLowerCase());
}

function render_shows(shows) {
  let ret = "<h1>Shows</h1>";

  for (let [show_id, show] of shows) {
    ret += '<a href="/show.html?show_id=' + show_id + '">';
    ret += show.name;
    ret += "</a>";
    ret += "<br>";
  }
  return ret;
}

async function init() {
  let shows = await get_shows();

  shows = Object.entries(shows);
  sort_shows_by_name(shows);

  const shows_list = document.getElementById("shows_list");
  shows_list.innerHTML = render_shows(shows);
}

init();
