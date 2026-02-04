---
name: nextjs-mobile-responsive
description: "Mobile-first responsive design rules for Next.js/React/Tailwind frontends. Ensures elements never overflow, buttons are touch-friendly, and layouts adapt correctly to all screen sizes."
auto_inject_for: ["web"]
---

# Next.js Mobile Responsive Design Rules

These rules MUST be followed when developing any Next.js, React, or Tailwind CSS frontend to ensure perfect mobile usability.

---

## Core Principle: Mobile-First, Overflow-Free

**Every element MUST fit its container on all screen sizes (320px minimum).** Users must be able to browse perfectly on mobile devices.

---

## Rule 1: No Fixed Widths Without Max-Width

**NEVER use fixed widths that can overflow the container.**

```tsx
// BAD - Will overflow on small screens
<div className="w-[400px]">Content</div>
<button className="px-8">Long Button Text Here</button>

// GOOD - Respects container bounds
<div className="w-full max-w-[400px]">Content</div>
<button className="w-full sm:w-auto px-4 sm:px-8">Long Button Text Here</button>
```

---

## Rule 2: Flex Containers Must Wrap or Stack

**Use `flex-wrap` or stack on mobile with `flex-col sm:flex-row`.**

```tsx
// BAD - Buttons will overflow on mobile
<div className="flex gap-3">
  <button>Add to Cart</button>
  <button>Wishlist</button>
  <button>Compare</button>
  <button>Price Alert</button>
</div>

// GOOD - Stacks on mobile, row on desktop
<div className="flex flex-col gap-2 sm:flex-row sm:flex-wrap sm:gap-3">
  <button className="w-full sm:w-auto">Add to Cart</button>
  <button>Wishlist</button>
  <button>Compare</button>
  <button>Price Alert</button>
</div>

// GOOD - Grid for equal distribution
<div className="grid grid-cols-2 gap-2 sm:flex sm:gap-3">
  <button>Wishlist</button>
  <button>Compare</button>
</div>
```

---

## Rule 3: Touch Targets Must Be 44px Minimum

**All interactive elements need minimum 44x44px touch target on mobile.**

```tsx
// BAD - Too small for fingers
<button className="p-1">
  <Icon className="h-4 w-4" />
</button>

// GOOD - Proper touch target
<button className="min-h-[44px] min-w-[44px] p-2 touch-manipulation">
  <Icon className="h-5 w-5" />
</button>
```

---

## Rule 4: Text Must Truncate or Wrap

**Long text should never push layouts wider than viewport.**

```tsx
// BAD - Long product name breaks layout
<h3>{product.name}</h3>

// GOOD - Truncates with ellipsis
<h3 className="truncate">{product.name}</h3>

// GOOD - Limits to 2 lines
<h3 className="line-clamp-2">{product.name}</h3>

// GOOD - Button text truncates
<button className="flex items-center gap-2">
  <Icon className="flex-shrink-0 h-5 w-5" />
  <span className="truncate">Wishlist</span>
</button>
```

---

## Rule 5: Images Must Have Proper Sizing

**Images should never overflow their containers.**

```tsx
// BAD - Fixed dimensions
<Image width={500} height={500} />

// GOOD - Responsive with aspect ratio
<div className="relative aspect-square w-full">
  <Image
    fill
    sizes="(max-width: 640px) 50vw, (max-width: 1024px) 33vw, 25vw"
    className="object-contain"
  />
</div>
```

---

## Rule 5b: Image Galleries & Thumbnails

**Image pickers and galleries must have mobile-friendly touch targets.**

### Thumbnail Grids/Rows

```tsx
// BAD - Thumbnails too small for touch
<button className="h-12 w-12">
  <Image fill sizes="48px" />
</button>

// GOOD - Minimum 64px with proper touch target
<button className="relative h-16 w-16 sm:h-18 sm:w-18 md:h-20 md:w-20
                   min-h-[64px] min-w-[64px] shrink-0
                   touch-manipulation rounded-lg border-2">
  <Image fill sizes="(max-width: 640px) 64px, (max-width: 768px) 72px, 80px" />
</button>
```

### Horizontal Thumbnail Scrolling

```tsx
// GOOD - Scrollable thumbnails with padding for touch
<div className="flex gap-2 sm:gap-3 overflow-x-auto pb-2 -mx-1 px-1
               scrollbar-none snap-x snap-mandatory">
  {images.map((img, i) => (
    <button
      key={img.id}
      className="relative h-16 w-16 min-h-[64px] min-w-[64px]
                 shrink-0 snap-start touch-manipulation"
    >
      <Image fill />
    </button>
  ))}
</div>
```

### Lightbox/Fullscreen Gallery

```tsx
// GOOD - Full screen lightbox for mobile
<div className="fixed inset-0 z-[100] bg-black/95 sm:bg-black/90">
  {/* Top bar with counter and close */}
  <div className="absolute left-0 right-0 top-0 z-10 flex items-center
                  justify-between p-3 sm:p-4">
    <div className="text-sm text-white/80">1 / 5</div>
    <button className="flex h-11 w-11 items-center justify-center
                       rounded-full bg-white/10 touch-manipulation">
      &times;
    </button>
  </div>

  {/* Navigation buttons - rounded, visible on dark background */}
  <button className="absolute left-2 top-1/2 -translate-y-1/2
                     flex h-12 w-12 sm:h-14 sm:w-14 items-center justify-center
                     rounded-full bg-white/10 touch-manipulation">
    &#8249;
  </button>

  {/* Main image - full viewport height minus chrome */}
  <div className="relative h-[calc(100dvh-120px)] w-full sm:h-[80dvh] sm:w-[90vw]">
    <Image fill sizes="(max-width: 640px) 100vw, 90vw" className="object-contain" />
  </div>

  {/* Bottom thumbnails with safe area padding */}
  <div className="absolute bottom-0 left-0 right-0 flex justify-center gap-2
                  bg-gradient-to-t from-black/50 p-3
                  pb-[max(12px,env(safe-area-inset-bottom))]">
    {/* 48px thumbnails for lightbox */}
  </div>
</div>
```

### Image Picker Component

```tsx
// GOOD - Grid-based image picker for selecting multiple
<div className="grid grid-cols-3 gap-2 sm:grid-cols-4 md:grid-cols-5 lg:grid-cols-6">
  {images.map((img) => (
    <button
      key={img.id}
      className="relative aspect-square min-h-[80px] overflow-hidden
                 rounded-lg border-2 touch-manipulation
                 active:scale-95 transition-transform"
    >
      <Image fill sizes="(max-width: 640px) 33vw, 20vw" className="object-cover" />
      {selected && (
        <div className="absolute inset-0 bg-primary/20 flex items-center justify-center">
          <CheckIcon className="h-8 w-8 text-primary" />
        </div>
      )}
    </button>
  ))}
</div>
```

---

## Rule 6: Use CSS Grid for Card Layouts

**Grid is more reliable than flex for card layouts.**

```tsx
// GOOD - Responsive grid
<div className="grid grid-cols-2 gap-3 sm:grid-cols-3 lg:grid-cols-4">
  {products.map(p => <ProductCard key={p.id} product={p} />)}
</div>

// GOOD - Single column on mobile
<div className="grid grid-cols-1 gap-4 sm:grid-cols-2 lg:grid-cols-3">
  {items.map(item => <Card key={item.id} item={item} />)}
</div>
```

---

## Rule 7: Horizontal Scroll for Data Tables

**Tables that don't fit should scroll horizontally, not overflow.**

```tsx
// GOOD - Scrollable table container
<div className="overflow-x-auto -mx-4 px-4 sm:mx-0 sm:px-0">
  <table className="min-w-full">...</table>
</div>
```

---

## Rule 8: Modal/Dialog Sizing

**Modals must not exceed viewport on mobile.**

```tsx
// GOOD - Full width on mobile, constrained on desktop
<div className="fixed inset-0 flex items-center justify-center p-4">
  <div className="w-full max-w-md max-h-[90vh] overflow-y-auto rounded-lg bg-white">
    {/* Modal content */}
  </div>
</div>
```

---

## Rule 9: Spacing Must Be Responsive

**Reduce spacing on mobile to fit more content.**

```tsx
// BAD - Same spacing everywhere
<div className="p-8 gap-6">

// GOOD - Tighter on mobile
<div className="p-4 gap-3 sm:p-6 sm:gap-4 lg:p-8 lg:gap-6">
```

---

## Rule 10: Forms Must Stack on Mobile

**Form inputs should stack vertically on mobile.**

```tsx
// GOOD - Stacked inputs
<form className="flex flex-col gap-4 sm:flex-row sm:items-end">
  <input className="w-full sm:flex-1" />
  <button className="w-full sm:w-auto">Submit</button>
</form>
```

---

## Quick Reference: Mobile Breakpoints

| Breakpoint | Width | Common Use |
|------------|-------|------------|
| (default)  | 0-639px | Mobile phones |
| `sm:` | 640px+ | Large phones, small tablets |
| `md:` | 768px+ | Tablets |
| `lg:` | 1024px+ | Laptops |
| `xl:` | 1280px+ | Desktops |
| `2xl:` | 1536px+ | Large desktops |

---

## Quick Reference: Common Patterns

### Button Group (Multiple Action Buttons)

```tsx
// Primary action full width, icons in row below
<div className="flex flex-col gap-2">
  <button className="w-full min-h-[44px] bg-primary text-white">
    Add to Cart
  </button>
  <div className="flex gap-2 justify-center">
    <button className="min-h-[40px] min-w-[40px] flex-1 max-w-[48px]">
      <HeartIcon />
    </button>
    <button className="min-h-[40px] min-w-[40px] flex-1 max-w-[48px]">
      <CompareIcon />
    </button>
  </div>
</div>
```

### Price + Action Layout

```tsx
<div className="flex flex-col gap-3 sm:flex-row sm:items-center sm:justify-between">
  <div className="text-lg font-bold">$99.00</div>
  <button className="w-full sm:w-auto">Buy Now</button>
</div>
```

### Card Footer Actions

```tsx
<div className="grid grid-cols-2 gap-2 sm:flex sm:gap-3">
  <button className="min-h-[44px]">Action 1</button>
  <button className="min-h-[44px]">Action 2</button>
</div>
```

---

## Validation Checklist

Before marking frontend work complete, verify:

- [ ] Tested at 375px width (iPhone SE/mini size)
- [ ] Tested at 320px width (minimum supported)
- [ ] No horizontal scrollbar on page body
- [ ] All buttons/links have 44px minimum touch target
- [ ] Long text truncates or wraps properly
- [ ] Forms are usable on mobile
- [ ] Modals don't exceed viewport
- [ ] Images scale properly
- [ ] Image galleries have 64px+ thumbnails
- [ ] Lightbox is full-screen on mobile with proper nav buttons
- [ ] Image pickers have proper touch targets and visual feedback

---

## Common Tailwind Classes for Responsive Design

| Pattern | Classes |
|---------|---------|
| Stack mobile, row desktop | `flex flex-col sm:flex-row` |
| Full width mobile, auto desktop | `w-full sm:w-auto` |
| Touch-friendly button | `min-h-[44px] min-w-[44px] touch-manipulation` |
| Prevent text overflow | `truncate` or `line-clamp-2` |
| Icon shouldn't shrink | `flex-shrink-0` |
| Responsive padding | `p-3 sm:p-4 lg:p-6` |
| Responsive gap | `gap-2 sm:gap-3 lg:gap-4` |
| Hide on mobile | `hidden sm:block` |
| Show only on mobile | `sm:hidden` |
| Container with max-width | `w-full max-w-md` |
| Scrollable thumbnails | `flex gap-2 overflow-x-auto scrollbar-none snap-x` |
| Image thumbnail | `h-16 w-16 min-h-[64px] min-w-[64px] shrink-0` |
| Lightbox overlay | `fixed inset-0 z-[100] bg-black/95` |
| Safe area bottom padding | `pb-[max(12px,env(safe-area-inset-bottom))]` |
| Rounded nav button | `rounded-full bg-white/10 backdrop-blur-sm` |
