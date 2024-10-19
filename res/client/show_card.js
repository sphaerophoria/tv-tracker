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

  const progress = [];
  const skipped_progress = [];
  if (show.episodes_aired !== undefined && show.episodes_watched.length > 0) {
    for (const num_watched of show.episodes_watched) {
      progress.push(num_watched / show.episodes_aired);
    }

    for (const num_skipped of show.episodes_skipped) {
      skipped_progress.push(num_skipped / show.episodes_aired);
    }
  } else if (show.watched === true) {
    // movie
    progress.push(1.0);
  } else {
    progress.push(0.0);
  }

  for (let i = 0; i < progress.length; ++i) {
    const progress_elem = progress[i];
    const skipped_elem = skipped_progress[i];
    const progress_div = document.createElement("div");
    progress_div.classList.add("show-progress");
    card_content.appendChild(progress_div);

    const watched_div = document.createElement("div");
    watched_div.classList.add("num-watched");
    progress_div.appendChild(watched_div);

    if (progress_elem !== undefined) {
      watched_div.style.width = "" + progress_elem * 100 + "%";
    }

    const skipped_div = document.createElement("div");
    skipped_div.classList.add("num-skipped");
    progress_div.appendChild(skipped_div);

    if (skipped_elem !== undefined) {
      skipped_div.style.width = "" + skipped_elem * 100 + "%";
    }

    const unwatched_div = document.createElement("div");
    unwatched_div.classList.add("num-unwatched");
    unwatched_div.style.width = "auto";
    progress_div.appendChild(unwatched_div);
  }

  const name = document.createElement("p");
  name.innerText = show.name;
  if (show.year !== null) {
    name.innerText += " (" + show.year + ")";
  }
  name.classList.add("show-name");
  card_content.appendChild(name);

  return card;
}
