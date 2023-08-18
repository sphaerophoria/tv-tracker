"use strict";

import {
  get_show_episodes,
  get_show,
  put_show,
  put_episode,
  delete_show,
  get_ratings,
} from "./http.js";

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

async function remove_show(show_id) {
  await delete_show(show_id);
  window.location.href = "/shows.html";
}

class ShowPage {
  constructor(show, episodes, ratings) {
    this.show = show;
    this.episodes = episodes;
    this.ratings = ratings;

    const mark_watched_button = document.getElementById("mark-all-watched");
    mark_watched_button.onclick = () => this.mark_aired_watched();

    const mark_unwatched_button = document.getElementById("mark-all-unwatched");
    mark_unwatched_button.onclick = () => this.mark_all_unwatched();

    const remove_show_button = document.getElementById("remove-show");
    remove_show_button.onclick = () => remove_show(this.show.id);
  }

  async mark_aired_watched() {
    const now = new Date(Date.now());
    let date_string = now.toISOString().substring(0, 10);
    let promises = [];
    for (const episode_id in this.episodes) {
      const episode = this.episodes[episode_id];
      if (now > Date.parse(episode.airdate)) {
        let new_episode = window.structuredClone(episode);
        new_episode.watch_date = date_string;
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
      new_episode.watch_date = null;
      promises.push(this.put_episode(new_episode));
    }

    await Promise.all(promises);
    this.render_show();
  }

  async set_show_watch_status(episode_id) {
    let episode = window.structuredClone(this.episodes[episode_id]);

    if (episode.watch_date != null) {
      episode.watch_date = null;
    } else {
      const now = new Date(Date.now());
      let date_string = now.toISOString().substring(0, 10);
      episode.watch_date = date_string;
    }

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
    let response = await put_episode(episode);
    this.episodes[response.id] = response;
  }

  render_show() {
    const title_node = document.getElementById("show-title");
    title_node.innerHTML = "<h1>" + this.show.name + "</h1>";

    const set_pause_button = document.getElementById("pause");

    if (this.show.pause_status === true) {
      set_pause_button.value = "Unpause show";
    } else {
      set_pause_button.value = "Pause show";
    }
    set_pause_button.onclick = () => this.pause_show();

    const div = document.getElementById("show-seasons");
    div.innerHTML = "";

    let season_episodes = group_episodes_by_seasons(this.episodes);
    let today = Date.now();

    const ratings = document.getElementById("ratings");
    ratings.onselect = null;

    const no_rating_option = document.getElementById("no-rating-option");
    no_rating_option.rating_id = null;
    ratings.innerHTML = "";
    ratings.add(no_rating_option);

    for (const rating of Object.values(this.ratings)) {
      let option = document.createElement("option");
      option.rating_id = rating.id;
      option.innerText = rating.name;
      ratings.add(option);

      if (rating.id == this.show.rating_id) {
        ratings.selectedIndex = ratings.length - 1;
      }
    }

    ratings.onchange = (e) => {
      let rating_id = ratings.options[ratings.selectedIndex].rating_id;
      let new_show = window.structuredClone(this.show);
      new_show.rating_id = rating_id;
      this.put_show(new_show);
    };

    for (const [season, episodes] of season_episodes) {
      const season_header = document.createElement("h2");
      season_header.innerHTML = "Season " + season;
      div.appendChild(season_header);
      for (let [episode_id, episode] of episodes) {
        let aired_class = "unaired";
        if (episode.airdate !== null && Date.parse(episode.airdate) < today) {
          aired_class = "aired";
        }

        let watched_class = "unwatched";
        const episode_watched = episode.watch_date != null;
        if (episode_watched) {
          watched_class = "watched";
        }

        const link = document.createElement("a");
        link.href = "javascript:void(0)";
        link.classList.add(aired_class);
        link.classList.add(watched_class);
        link.onclick = () => this.set_show_watch_status(episode.id);

        div.appendChild(link);
        div.appendChild(document.createElement("br"));
        const link_text = document.createTextNode("");
        link.appendChild(link_text);

        if (episode.airdate !== null) {
          link_text.appendData("" + episode.airdate);
        } else {
          link_text.appendData("TBD");
        }
        link_text.appendData(
          " Episode " + episode.episode + ": " + episode.name
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
