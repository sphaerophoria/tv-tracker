"use strict";

import { get_shows } from "./http.js";

function sort_shows_by_name(shows) {
  shows.sort((a, b) => a[1].name.toLowerCase() > b[1].name.toLowerCase());
}

function render_shows(shows) {
  sort_shows_by_name(shows);

  let ret = "";
  for (const item of shows) {
    let show_id, show;
    [show_id, show] = item;
    ret += "<a href=/show.html?show_id=" + show_id + ">";
    ret += show.name;
    ret += "</a>";
    ret += "<br>";
  }
  return ret;
}

async function populate_episodes() {
  let shows = await get_shows();

  let next_up_shows = [];
  let finished_shows = [];
  let unstarted_shows = [];
  let paused_shows = [];

  for (const show_id in shows) {
    const show = shows[show_id];

    if (show.pause_status === true) {
      paused_shows.push([show_id, show]);
    } else if (show.watch_status == "finished") {
      finished_shows.push([show_id, show]);
    } else if (show.watch_status == "in_progress") {
      next_up_shows.push([show_id, show]);
    } else if (show.watch_status == "unstarted") {
      unstarted_shows.push([show_id, show]);
    } else {
      throw new Error("Invalid show status " + show_status);
    }
  }

  const finished_div = document.getElementById("finished");
  finished_div.innerHTML = render_shows(finished_shows);

  const unstarted_div = document.getElementById("unstarted");
  unstarted_div.innerHTML = render_shows(unstarted_shows);

  const next_up_div = document.getElementById("next-up");
  next_up_div.innerHTML = render_shows(next_up_shows);

  const paused_div = document.getElementById("paused");
  paused_div.innerHTML = render_shows(paused_shows);
}

async function init() {
  populate_episodes();
}

init();
