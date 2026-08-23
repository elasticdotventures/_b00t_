export { TenantNode } from "./tenant-do";

export default {
  async fetch(): Promise<Response> {
    return new Response("not implemented", { status: 501 });
  },
};
