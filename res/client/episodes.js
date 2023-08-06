"use strict";

import { request_episodes, request_shows } from "./http.js";

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

async function populate_episodes() {
  let shows = await request_shows();
  let show_ids = [];
  let promises = [];
  for (let show_id in shows) {
    show_ids.push(show_id);
    promises.push(request_episodes(show_id));
  }

  let show_episodes = await Promise.all(promises);
  let episodes_json = show_ids.map((show_id, idx) => [
    show_id,
    show_episodes[idx],
  ]);
  episodes_json = new Map(episodes_json);

  const upcoming_div = document.getElementById("upcoming_episodes");
  let rendered = "";
  const today = Date.now();
  for (const show_id of episodes_json.keys()) {
    const episodes = episodes_json.get(show_id);
    const show_name = shows[show_id].name;
    const next_episode = get_next_episode(episodes, today);
    if (next_episode === null) {
      rendered +=
        "<li>" + show_name + " has no new episodes scheduled" + "</li>";
    } else {
      rendered +=
        "<li>" +
        show_name +
        " S" +
        next_episode.season +
        "E" +
        next_episode.episode +
        " on " +
        next_episode.airdate +
        "</li>";
    }
  }

  upcoming_div.innerHTML = rendered;
}

async function init() {
  populate_episodes();
}

init();
