import { get_movie, put_movie, delete_movie, get_ratings } from "./http.js";

import { create_ratings_selector } from "./ratings_widget.js";

class MoviePage {
  constructor(movie, ratings) {
    this.movie = movie;
    this.ratings = ratings;

    const movie_title = document.getElementById("movie-title");
    movie_title.innerText = this.movie.name;

    const movie_poster = document.getElementById("poster");
    movie_poster.src = "images/" + this.movie.image;

    const remove_button = document.getElementById("remove-movie");
    remove_button.onclick = () => this.delete_movie();

    const watch_button = document.getElementById("watch-button");
    watch_button.onclick = () => this.toggle_watch_status();
  }

  async delete_movie() {
    await delete_movie(this.movie.id);
    window.location.href = "watch_list.html?movies=true";
  }

  async toggle_watch_status() {
    const movie = window.structuredClone(this.movie);
    movie.watched = !movie.watched;
    console.log(movie);
    this.movie = await put_movie(movie);
    console.log(this.movie);
    this.render();
  }

  async put_movie(movie) {
    const returned_movie = await put_movie(movie);
    this.movie = returned_movie;
    this.render();
  }

  render() {
    const watch_button = document.getElementById("watch-button");
    if (this.movie.watched) {
      watch_button.value = "Mark unwatched";
    } else {
      watch_button.value = "Mark watched";
    }

    const ratings_div = document.getElementById("ratings");
    ratings_div.innerHTML = "";
    create_ratings_selector(this.movie, this.ratings, ratings_div, (e) => {
      const ratings_selector = e.originalTarget;
      let rating_id =
        ratings_selector.options[ratings_selector.selectedIndex].rating_id;
      let new_movie = window.structuredClone(this.movie);
      new_movie.rating_id = rating_id;
      this.put_movie(new_movie);
    });
  }
}

async function init() {
  const my_url = new URL(document.URL);
  const movie_id = my_url.searchParams.get("movie_id");
  const movie_promise = get_movie(movie_id);
  const ratings_promise = get_ratings();

  let movie, ratings;
  [movie, ratings] = await Promise.all([movie_promise, ratings_promise]);
  const page = new MoviePage(movie, ratings);
  page.render();
}

init();
