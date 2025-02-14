"use strict";

import {
  get_show_episodes,
  get_show,
  put_show,
  put_episode,
  delete_show,
  get_ratings,
} from "./http.js";

import { create_ratings_selector } from "./ratings_widget.js";

function page_show_id() {
  const params = new URLSearchParams(document.location.search);
  return params.get("show_id");
}

function group_episodes_by_seasons(episodes) {
  let season_episodes = new Map();

  for (const i in episodes) {
    const episode = episodes[i];
    if (!season_episodes.has(episode.season)) {
      season_episodes.set(episode.season, []);
    }

    let this_season_episodes = season_episodes.get(episode.season);
    this_season_episodes.push([i, episode]);
  }

  return season_episodes;
}

function show_youtube_search_url(show) {
  let query = show.name.replace(" ", "+");
  query += "+trailer";
  return "https://www.youtube.com/results?search_query=" + query;
}

async function remove_show(show_id) {
  await delete_show(show_id);
  window.location.href = "watch_list.html";
}

// This is so that we can index into a new playthrough without worrying about
// going out of bounds
function append_to_watch_status(episode) {
  episode.watch_status.push("Unwatched");
}

function append_to_watch_statuses(episodes) {
  for (const episode_id in episodes) {
    append_to_watch_status(episodes[episode_id]);
  }
}

class ShowPage {
  constructor(show, episodes, ratings) {
    this.show = show;
    this.episodes = episodes;
    this.ratings = ratings;

    append_to_watch_statuses(this.episodes);

    const mark_watched_button = document.getElementById("mark-all-watched");
    mark_watched_button.onclick = () => this.mark_aired_watched();

    const mark_unwatched_button = document.getElementById("mark-all-unwatched");
    mark_unwatched_button.onclick = () => this.mark_all_unwatched();

    const remove_show_button = document.getElementById("remove-show");
    remove_show_button.onclick = () => remove_show(this.show.id);

    const playthrough_input = document.getElementById(
      "multi-playthough-selector",
    );

    // NOTE: This means if we try to start two playthroughs back to back we
    // have to refresh the page. Not ideal, but in practice not a problem
    playthrough_input.max = this.show.episodes_watched.length + 1;

    playthrough_input.value = this.show.episodes_watched.length;
    this.playthrough = Number(playthrough_input.value) - 1;
    playthrough_input.oninput = (ev) => {
      this.playthrough = Number(ev.target.value) - 1;
      this.render_show();
    };
  }

  async mark_aired_watched() {
    const now = new Date(Date.now());
    let date_string = now.toISOString().substring(0, 10);
    let promises = [];
    for (const episode_id in this.episodes) {
      const episode = this.episodes[episode_id];
      if (now > Date.parse(episode.airdate)) {
        let new_episode = window.structuredClone(episode);
        new_episode.watch_status[this.playthrough] = {
          Watched: date_string,
        };
        promises.push(this.put_episode(new_episode));
      }
    }

    await Promise.all(promises);
    this.render_show();
  }

  async mark_all_unwatched() {
    let promises = [];
    for (const episode_id in this.episodes) {
      let new_episode = window.structuredClone(this.episodes[episode_id]);
      new_episode.watch_status[this.playthrough] = "Unwatched";
      promises.push(this.put_episode(new_episode));
    }

    await Promise.all(promises);
    this.render_show();
  }

  async set_episode_unwatched(episode_id) {
    let episode = window.structuredClone(this.episodes[episode_id]);
    episode.watch_status[this.playthrough] = "Unwatched";
    await this.put_episode(episode);
    this.render_show();
  }

  async set_episode_skipped(episode_id) {
    let episode = window.structuredClone(this.episodes[episode_id]);
    episode.watch_status[this.playthrough] = "Skipped";
    await this.put_episode(episode);
    this.render_show();
  }

  async set_episode_watched(episode_id) {
    let episode = window.structuredClone(this.episodes[episode_id]);

    const now = new Date(Date.now());
    let date_string = now.toISOString().substring(0, 10);
    episode.watch_status[this.playthrough] = {
      Watched: date_string,
    };

    await this.put_episode(episode);
    this.render_show();
  }

  async pause_show() {
    let new_show = window.structuredClone(this.show);
    new_show.pause_status = !new_show.pause_status;
    await this.put_show(new_show);
    this.render_show();
  }

  async put_show(show) {
    let response = await put_show(show);
    this.show = response;
  }

  async put_episode(episode) {
    // We have injected an extra playthrough for UI convenience. This should
    // _not_ make it back to the server. We need to clamp the length of the
    // watched field to the number of playthroughs UNLESS we are starting
    // a new playthrough

    const expected_num_playthroughs = this.show.episodes_watched.length;
    const clone = structuredClone(episode);
    if (clone.watch_status[expected_num_playthroughs - 1] === "Unwatched") {
      clone.watch_status.pop();
    }

    let response = await put_episode(clone);
    append_to_watch_status(response);
    this.episodes[response.id] = response;
  }

  render_show() {
    const poster = document.getElementById("poster");
    poster.src = "images/" + this.show.image;

    const title_node = document.getElementById("show-title");
    title_node.innerHTML = "<h1>" + this.show.name + "</h1>";

    const set_pause_button = document.getElementById("pause");

    if (this.show.pause_status === true) {
      set_pause_button.value = "Unpause show";
    } else {
      set_pause_button.value = "Pause show";
    }
    set_pause_button.onclick = () => this.pause_show();

    const youtube_link = document.getElementById("youtube-link");
    youtube_link.href = show_youtube_search_url(this.show);

    const div = document.getElementById("show-seasons");
    div.innerHTML = "";

    let season_episodes = group_episodes_by_seasons(this.episodes);
    let today = Date.now();

    const ratings_parent = document.getElementById("ratings");
    ratings_parent.innerHTML = "";
    create_ratings_selector(this.show, this.ratings, ratings_parent, (e) => {
      const ratings_selector = e.originalTarget;
      let rating_id =
        ratings_selector.options[ratings_selector.selectedIndex].rating_id;
      let new_show = window.structuredClone(this.show);
      new_show.rating_id = rating_id;
      this.put_show(new_show);
    });

    for (const [season, episodes] of season_episodes) {
      const season_header = document.createElement("h2");
      season_header.innerHTML = "Season " + season;
      div.appendChild(season_header);
      for (let [episode_id, episode] of episodes) {
        let aired_class = "unaired";
        if (episode.airdate !== null && Date.parse(episode.airdate) < today) {
          aired_class = "aired";
        }

        const episode_holder = document.createElement("div");
        episode_holder.classList.add("episode_holder");

        const unwatched_button = document.createElement("input");
        unwatched_button.type = "image";
        unwatched_button.src = "img/unwatched.png";
        unwatched_button.classList.add("watched_button");
        episode_holder.appendChild(unwatched_button);
        unwatched_button.onclick = () => this.set_episode_unwatched(episode.id);

        const skipped_button = document.createElement("input");
        skipped_button.type = "image";
        skipped_button.src = "img/skip_episode.png";
        skipped_button.classList.add("watched_button");
        skipped_button.onclick = () => this.set_episode_skipped(episode.id);
        episode_holder.appendChild(skipped_button);

        const watched_button = document.createElement("input");
        watched_button.type = "image";
        watched_button.src = "img/watched.png";
        watched_button.classList.add("watched_button");
        watched_button.onclick = () => this.set_episode_watched(episode.id);
        episode_holder.appendChild(watched_button);

        let watched_class = "unknown";
        if (episode.watch_status[this.playthrough].Watched != null) {
          watched_button.classList.add("active");
          watched_class = "watched";
        } else if (episode.watch_status[this.playthrough] == "Skipped") {
          skipped_button.classList.add("active");
          watched_class = "skipped";
        } else {
          unwatched_button.classList.add("active");
          watched_class = "unwatched";
        }

        const link = document.createElement("div");
        link.id = "episode-" + episode_id;
        link.classList.add(aired_class);
        link.classList.add(watched_class);

        episode_holder.appendChild(link);
        div.appendChild(episode_holder);

        const link_text = document.createTextNode("");
        link.appendChild(link_text);

        if (episode.airdate !== null) {
          link_text.appendData("" + episode.airdate);
        } else {
          link_text.appendData("TBD");
        }
        link_text.appendData(
          " Episode " + episode.episode + ": " + episode.name,
        );
      }
    }
  }
}

async function init() {
  const show_id = parseInt(page_show_id());

  const show_promise = get_show(show_id);
  const episodes_promise = get_show_episodes(show_id);
  const ratings_promise = get_ratings();

  let show, episodes, ratings;
  [show, episodes, ratings] = await Promise.all([
    show_promise,
    episodes_promise,
    ratings_promise,
  ]);

  const page = new ShowPage(show, episodes, ratings);
  page.render_show();
}

init();
