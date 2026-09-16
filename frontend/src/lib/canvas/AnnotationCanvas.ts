/**
 * Canvas 2D 标注引擎。
 * 四层画布：底图 → 遮罩 → 绘制中路径 → UI 叠加。
 */
export class AnnotationCanvas {
  private layers: HTMLCanvasElement[] = [];
  private ctxs: CanvasRenderingContext2D[] = [];
  private scale = 1;
  private offsetX = 0;
  private offsetY = 0;

  constructor(container: HTMLElement) {
    for (let i = 0; i < 4; i++) {
      const c = document.createElement("canvas");
      c.style.position = "absolute";
      c.style.top = "0";
      c.style.left = "0";
      container.appendChild(c);
      this.layers.push(c);
      this.ctxs.push(c.getContext("2d")!);
    }
  }

  get imageLayer() { return this.ctxs[0]; }
  get maskLayer() { return this.ctxs[1]; }
  get drawLayer() { return this.ctxs[2]; }
  get overlayLayer() { return this.ctxs[3]; }

  setImage(src: string) {
    const img = new Image();
    img.onload = () => {
      for (const c of this.layers) { c.width = img.width; c.height = img.height; }
      this.imageLayer.drawImage(img, 0, 0);
    };
    img.src = src;
  }

  resize(w: number, h: number) {
    for (const c of this.layers) { c.width = w; c.height = h; }
  }

  destroy() {
    for (const c of this.layers) c.remove();
  }
}
