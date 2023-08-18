import { get_ratings, put_rating, put_ratings, delete_rating } from "./http.js";

class RatingsPage {
  constructor(ratings) {
    this.ratings = ratings;
  }

  async update_rating_name(rating_id, item) {
    const new_rating = window.structuredClone(this.ratings[rating_id]);
    new_rating.name = item.innerText;
    this.ratings[rating_id] = await put_rating(new_rating);
    this.render();
  }

  async swap_priorities(a, b) {
    let a_new = window.structuredClone(a);
    let b_new = window.structuredClone(b);

    let tmp = a_new.priority;
    a_new.priority = b_new.priority;
    b_new.priority = tmp;

    this.ratings[a_new.id] = await put_rating(a_new);
    this.ratings[b_new.id] = await put_rating(b_new);

    this.render();
  }

  async add_rating(name) {
    let response = await put_ratings({
      name: name,
    });
    this.ratings[response.id] = response;
    this.render();
  }

  async delete_rating(rating_id) {
    await delete_rating(rating_id);
    delete this.ratings[rating_id];
    this.render();
  }

  render() {
    let ratings = Object.values(this.ratings);
    ratings.sort((a, b) => a.priority >= b.priority);
    let div = document.getElementById("ratings-div");
    div.innerHTML = "";

    for (const rating of ratings) {
      const elem_div = document.createElement("div");
      elem_div.classList.add("rating-div");
      div.appendChild(elem_div);

      const header = document.createElement("h2");
      header.contentEditable = true;
      header.innerHTML = rating.name;

      header.onkeydown = (event) => {
        if (event.keyCode == 13) {
          event.preventDefault();
          header.blur();
        }
      };

      header.onblur = (event) => {
        this.update_rating_name(rating.id, header);
      };

      elem_div.appendChild(header);

      const up_button = document.createElement("input");
      up_button.type = "button";
      up_button.value = "Up";
      up_button.onclick = () => {
        let ratings = Object.values(this.ratings);
        let target_item = find_lower_priority(ratings, rating.priority);
        if (target_item !== null) {
          this.swap_priorities(rating, target_item);
        }
      };
      elem_div.appendChild(up_button);

      const down_button = document.createElement("input");
      down_button.type = "button";
      down_button.value = "Down";
      down_button.onclick = () => {
        let ratings = Object.values(this.ratings);
        let target_item = find_higher_priority(ratings, rating.priority);
        if (target_item !== null) {
          this.swap_priorities(rating, target_item);
        }
      };
      elem_div.appendChild(down_button);

      const delete_button = document.createElement("input");
      delete_button.type = "button";
      delete_button.value = "Delete";
      delete_button.onclick = () => {
        this.delete_rating(rating.id);
      };
      elem_div.appendChild(delete_button);
    }

    const elem_div = document.createElement("div");
    elem_div.classList.add("rating-div");
    div.appendChild(elem_div);

    let new_rating_name = document.createElement("input");
    new_rating_name.type = "text";
    new_rating_name.placeholder = "Add a rating";
    elem_div.appendChild(new_rating_name);

    let new_rating_add_button = document.createElement("input");
    new_rating_add_button.type = "button";
    new_rating_add_button.value = "Add";
    new_rating_add_button.onclick = () => {
      this.add_rating(new_rating_name.value);
    };
    elem_div.appendChild(new_rating_add_button);
  }
}

function find_lower_priority(array, target_priority) {
  let ret = array.reduce((acc, current) => {
    if (current.priority >= target_priority) {
      return acc;
    }

    if (acc === null) {
      return current;
    }

    if (acc.priority < current.priority) {
      return current;
    }

    return acc;
  }, null);

  return ret;
}

function find_higher_priority(array, target_priority) {
  let ret = array.reduce((acc, current) => {
    if (current.priority <= target_priority) {
      return acc;
    }

    if (acc === null) {
      return current;
    }

    if (acc.priority > current.priority) {
      return current;
    }

    return acc;
  }, null);

  return ret;
}

async function init() {
  const ratings = await get_ratings();
  const page = new RatingsPage(ratings);
  page.render();
}

init();
