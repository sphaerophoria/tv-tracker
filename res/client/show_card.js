"use strict";

// Used for both remote shows and full shows
export function render_card_element(show, href) {
  const card = document.createElement("div");
  card.classList.add("show-card");

  if (show.pause_status !== undefined && show.pause_status) {
    card.classList.add("paused");
  }

  const link = document.createElement("a");
  link.href = href;
  card.appendChild(link);

  if (show.image !== null) {
    const poster = document.createElement("img");
    if (typeof show.image == "string" && show.image.startsWith("http")) {
      poster.src = show.image;
    } else {
      poster.src = "/images/" + show.image;
    }
    link.appendChild(poster);
  } else {
    const poster = document.createElement("div");
    poster.classList.add("show-card-placeholder-image");
    link.appendChild(poster);
  }

  const progress_div = document.createElement("div");
  progress_div.classList.add("show-progress");
  link.appendChild(progress_div);

  const watched_div = document.createElement("div");
  watched_div.classList.add("num-watched");
  progress_div.appendChild(watched_div);

  let progress = 0;
  if (show.episodes_aired !== undefined && show.episodes_aired > 0) {
    progress = show.episodes_watched / show.episodes_aired;
  }

  if (progress !== undefined) {
    watched_div.style.width = "" + progress * 100 + "%";
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
  link.appendChild(name);

  return card;
}
