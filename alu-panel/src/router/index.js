import { createRouter, createWebHistory } from "vue-router";
import GamesView from "../views/GamesView.vue";
import UsersView from "../views/UsersView.vue";

const router = createRouter({
  history: createWebHistory("/admin/"),
  routes: [
    {
      path: "/",
      redirect: "/games",
    },
    {
      path: "/games",
      name: "games",
      component: GamesView,
    },
    {
      path: "/users",
      name: "users",
      component: UsersView,
    },
  ],
});

export default router;
