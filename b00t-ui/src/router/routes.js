const routes = [
  {
    path: '/',
    component: () => import('@/layouts/MainLayout.vue'),
    children: [
      { path: '', component: () => import('@/pages/IndexPage.vue') },
      { path: 'types', component: () => import('@/pages/TypesPage.vue') },
      { path: 'simulation', component: () => import('@/pages/SimulationPage.vue') },
      { path: 'visualizations', component: () => import('@/pages/VisualizationsPage.vue') },
      { path: 'second', component: () => import('@/pages/SecondPage.vue') }
    ],
  },

  // Always leave this as last one,
  // but you can also remove it
  {
    path: '/:catchAll(.*)*',
    component: () => import('@/pages/ErrorNotFound.vue'),
  }
]

export default routes
