"use strict";

import { get_shows, get_episodes } from "./http.js";

function date_to_string(date) {
  return date.toISOString().substring(0, 10);
}

function sort_by_date(episodes) {
  episodes.sort((a, b) => {
    return new Date(a.airdate) > new Date(b.airdate);
  });
}

async function init() {
  const today = new Date(Date.now());
  const month_start = new Date(today.getUTCFullYear(), today.getMonth(), 1);
  const month_end = new Date(today.getUTCFullYear(), today.getMonth() + 1, 0);

  let shows_promise = get_shows();
  let episodes_promise = get_episodes(
    date_to_string(month_start),
    date_to_string(month_end)
  );

  let shows, episodes;
  [shows, episodes] = await Promise.all([shows_promise, episodes_promise]);
  episodes = Object.values(episodes);
  sort_by_date(episodes);

  let date_it = null;
  let rendered = "";

  for (const episode of episodes) {
    if (date_it != episode.airdate) {
      date_it = episode.airdate;
      rendered += "<h1>" + date_it + "</h1>";
    }

    const show = shows[episode.show_id];
    rendered += "<a href=/show.html?show_id=" + episode.show_id + ">";
    rendered += show.name + ": S" + episode.season + "E" + episode.episode;
    rendered += "</a>";
    rendered += "<br>";
  }

  document.getElementById("calendar").innerHTML = rendered;
}

init();
