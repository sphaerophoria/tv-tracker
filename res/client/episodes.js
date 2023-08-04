function request_episodes() {
  const request = new Request("/episodes", {
    method: "GET",
  });
  return fetch(request);
}

function request_shows() {
  const request = new Request("/shows", {
    method: "GET",
  });
  return fetch(request);
}

function get_next_episode(episodes, today) {
  var next = null;
  var next_date = null;

  for (var i = 1; i < episodes.length; i++) {
    const this_date = Date.parse(episodes[i].airdate);

    if (today < this_date && (next_date === null || next_date > this_date)) {
      next = episodes[i];
      next_date = this_date;
    }
  }

  return next;
}

async function populate_episodes() {
  const response = await request_episodes();
  const json = await response.json();

  const shows_response = await request_shows();
  const shows_json = await shows_response.json();

  const upcoming_div = document.getElementById("upcoming_episodes");
  let rendered = "";
  const today = Date.now();
  for (const show_id in json) {
    const episodes = json[show_id];
    const show_name = shows_json[show_id].name;
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
