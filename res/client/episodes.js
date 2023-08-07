"use strict";

import { request_shows, request_shows_by_watch_status } from "./http.js";

function sort_shows_by_name(shows) {
  shows.sort((a, b) => a[1].name.toLowerCase() > b[1].name.toLowerCase());
}

function get_next_episode(episodes, today) {
  let next = null;
  let next_date = null;

  for (const i in episodes) {
    const this_date = Date.parse(episodes[i].airdate);

    if (today < this_date && (next_date === null || next_date > this_date)) {
      next = episodes[i];
      next_date = this_date;
    }
  }

  return next;
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

function remove_unaired_episodes(episodes, today) {
  return episodes.filter((episode) => {
    const show_date = Date.parse(episode.airdate);
    return show_date < today;
  });
}

async function populate_episodes() {
  let shows_promise = request_shows();
  let show_statuses_promise = request_shows_by_watch_status();

  let shows, show_statuses;
  [shows, show_statuses] = await Promise.all([
    shows_promise,
    show_statuses_promise,
  ]);

  let next_up_shows = [];
  let finished_shows = [];
  let unstarted_shows = [];

  for (const show_id in shows) {
    const show = shows[show_id];
    const show_status = show_statuses[show_id];

    if (show_status == "finished") {
      finished_shows.push([show_id, show]);
    } else if (show_status == "in_progress") {
      next_up_shows.push([show_id, show]);
    } else if (show_status == "unstarted") {
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
}

async function init() {
  populate_episodes();
}

init();
