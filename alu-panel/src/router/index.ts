import { createRouter, createWebHistory } from "vue-router";
import GamesView from "../views/GamesView.vue";
import UsersView from "../views/UsersView.vue";
import MetadataView from "../views/MetadataView.vue";
import TitleDBView from "../views/TitleDBView.vue";
import DownloadView from "../views/DownloadView.vue";

const router = createRouter({
  history: createWebHistory("/"),
  routes: [
    // {
    //   path: "/",
    //   redirect: "/games",
    // },
    {
      path: "/",
      name: "games",
      component: GamesView,
    },
    {
      path: "/users",
      name: "users",
      component: UsersView,
    },
    {
      path: "/titledb",
      name: "titledb",
      component: TitleDBView,
    },
    {
      path: "/metadata",
      name: "metadata",
      component: MetadataView,
    },
    {
      path: "/downloads",
      name: "downloads",
      component: DownloadView,
    },
  ],
});

export default router;
