"use strict";

import { request_episodes_aired_between } from "./http.js";

function date_to_string(date) {
  return date.toISOString().substring(0, 10);
}

function sort_by_date(episodes) {
  episodes.sort((a, b) => {
    return new Date(a.episode.airdate) > new Date(b.episode.airdate);
  });
}

async function init() {
  const today = new Date(Date.now());
  const month_start = new Date(today.getUTCFullYear(), today.getMonth(), 1);
  const month_end = new Date(today.getUTCFullYear(), today.getMonth() + 1, 0);

  let response = await request_episodes_aired_between(
    date_to_string(month_start),
    date_to_string(month_end)
  );
  sort_by_date(response.episodes);

  let date_it = null;
  let rendered = "";

  for (const episode of response.episodes) {
    if (date_it != episode.episode.airdate) {
      date_it = episode.episode.airdate;
      rendered += "<h1>" + date_it + "</h1>";
    }

    const show = response.shows[episode.show_id];
    rendered += "<a href=/show.html?show_id=" + episode.show_id + ">";
    rendered +=
      show.name +
      ": S" +
      episode.episode.season +
      "E" +
      episode.episode.episode;
    rendered += "</a>";
    rendered += "<br>";
    console.log(episode);
  }

  document.getElementById("calendar").innerHTML = rendered;
}

init();
