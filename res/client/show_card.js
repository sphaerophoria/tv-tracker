"use strict";

// Used for both remote shows and full shows
export function render_card_element(show, href) {
  const card = document.createElement("div");
  card.classList.add("show-card");

  if (show.pause_status !== undefined && show.pause_status) {
    card.classList.add("paused");
  }

  let card_content = card;
  if (href !== null) {
    const link = document.createElement("a");
    link.href = href;
    card.appendChild(link);
    card_content = link;
  }

  if (show.image !== null) {
    const poster = document.createElement("img");
    if (typeof show.image == "string" && show.image.startsWith("http")) {
      poster.src = show.image;
    } else {
      poster.src = "images/" + show.image;
    }
    card_content.appendChild(poster);
  } else {
    const poster = document.createElement("div");
    poster.classList.add("show-card-placeholder-image");
    card_content.appendChild(poster);
  }

  const progress_div = document.createElement("div");
  progress_div.classList.add("show-progress");
  card_content.appendChild(progress_div);

  const watched_div = document.createElement("div");
  watched_div.classList.add("num-watched");
  progress_div.appendChild(watched_div);

  let progress = 0;
  if (show.episodes_aired !== undefined && show.episodes_aired > 0) {
    progress = show.episodes_watched / show.episodes_aired;
  } else if (show.watched === true) {
    progress = 1.0;
  }

  if (progress !== undefined) {
    watched_div.style.width = "" + progress * 100 + "%";
  }

  const skipped_div = document.createElement("div");
  skipped_div.classList.add("num-skipped");
  progress_div.appendChild(skipped_div);

  let skipped_progress = 0;
  if (show.episodes_skipped !== undefined && show.episodes_aired > 0) {
    skipped_progress = show.episodes_skipped / show.episodes_aired;
  }
  if (skipped_progress !== undefined) {
    skipped_div.style.width = "" + skipped_progress * 100 + "%";
  }

  const unwatched_div = document.createElement("div");
  unwatched_div.classList.add("num-unwatched");
  unwatched_div.style.width = "auto";
  progress_div.appendChild(unwatched_div);

  const name = document.createElement("p");
  name.innerText = show.name;
  if (show.year !== null) {
    name.innerText += " (" + show.year + ")";
  }
  name.classList.add("show-name");
  card_content.appendChild(name);

  return card;
}
