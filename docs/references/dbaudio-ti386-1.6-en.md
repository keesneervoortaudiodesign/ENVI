# TI 386 (1.6 EN) — NoizCalc

> **Transcription note.** Markdown transcription of the d&b audiotechnik Technical
> Information **TI 386 (1.6 EN), "NoizCalc"** (source PDF: `dbaudio-ti386-1.6-en.pdf`,
> 18 pp, © 10/2019 d&b audiotechnik GmbH & Co. KG + SoundPLAN GmbH). Text extracted
> and reflowed; figure captions are retained as italic lines where the original had
> screenshots. Trademarks and content belong to their owners — included here as a
> reference for the ENVI project because NoizCalc is a directly analogous product
> (Nord2000 / ISO 9613-2 environmental-noise prediction with OSM/Google import, a
> Digital Ground Model, ground-effect/forest/building/wall modelling, and directive
> loudspeaker sources).

---

## Contents

- [Quick guide](#quick-guide)
  - [Getting started](#getting-started)
  - [Modelling](#modelling)
  - [Calculation](#calculation)
- [1. Introduction](#1-introduction)
- [2. Installation](#2-installation)
- [3. Calculating the environmental noise](#3-calculating-the-environmental-noise)
  - [3.1 Getting started](#31-getting-started)
  - [3.2 Modelling](#32-modelling)
  - [3.3 Calculation](#33-calculation)
  - [3.4 Graphic plot](#34-graphic-plot)
- [4. In-detail description](#4-in-detail-description)
- [5. Troubleshooting](#5-troubleshooting)
- [6. Legal notice](#6-legal-notice)

---

## Quick guide

This is a short step-by-step guide (1 page) how to prepare and calculate a noise prediction.

### Getting started

**Adjust NoizCalc reference point in ArrayCalc**
- Activate NoizCalc in **Settings > Advanced features**.
- Adjust the position of the reference point in the 3D plot at a representative point in the audience area at listening height (e.g. 1.7 m standing, 1.5 m sitting).
- Start NoizCalc. Create a new project: **File > New project**.
- Select a calculation standard (Nord2000 provides greater accuracy; use ISO 9613-2 when requested).
- Select default ground for your project: **Urban** for mostly paved grounds, **Rural** for countryside / fields.
- Integrate an ArrayCalc file in the project settings (button).

### Modelling

**Import geo data (bitmap, elevation data and objects)**
- New project: switch to **Editor** tab → map window opens.
- Projects with geo data: click map button in Editor tab (top right).
- Navigate to event location. Adjust viewport to obtain an overview of event site and possibly affected areas.
- Select service for bitmap: **Google Maps** (aerial image, map; with or without labels and points of interest) or **OpenStreetMap**.
- Adjust default building height. This value is used when no height data is available for import.

The import includes:
- Bitmap of selected viewport.
- Elevation data from Google Maps.
- Objects from OpenStreetMap (OSM) — Buildings, Ground effects and Forests.

Click the **Import** button to execute the import. A Digital Ground Model (DGM) is calculated and shown underneath the bitmap in the Editor. Objects are only imported onto the DGM. Check DGM and objects in the editor; use 3D map for heights.

Ground effects define the acoustical properties of grounds. Default ground: "urban" or "rural".

Buildings, Ground effects and Forests can be entered and edited (shape & height). If height data is not available, then:
- **Buildings:** user-defined default height is used.
- **Forests:** default height of 10 m is used.

The default right-angle drawing mode can be changed also while entering an object.

**Stage**
- Activate the Stage object by clicking the ArrayCalc symbol (black abacus). Place a stage on the bitmap using a left mouse click. Define its orientation (angle).
- **SPL at reference point and spectrum:** Select a spectrum from the pull-down menu or alternatively open the Emission library by clicking the library symbol for more options. Change **SPL at reference point** as needed.

### Calculation

The size of the noise color map (= result) is either determined by the min/max coordinates of all objects or by defining a calculation area. Click the abacus symbol to set the parameters for the calculation and **Run** to start it. After a successful calculation the result is automatically shown in the **Graphic plot** tab as a grid noise map. Click Signs and symbols, the level color scale or the length scale to customize the appearance of the result.

---

## 1. Introduction

This Technical Information explains the use of **NoizCalc** for noise predictions with sound systems defined in the **ArrayCalc** simulation software.

With NoizCalc you can calculate the noise impact on neighboring areas of sound systems at outdoor events. A sound system that has been defined in ArrayCalc is simply imported and positioned as one **stage** object. The quality of the simulation depends on the accuracy of the model: topography, terrain and different types of acoustically relevant objects have to be modelled to perform a realistic sound propagation calculation.

**What NoizCalc does.** NoizCalc calculates the sound propagation from the sources (sound system of one or more stages) to the grid points on a calculation area. The calculation is performed according to the chosen standard (**Nord2000** or **ISO 9613-2**) and takes into account directivity and interaction of complex loudspeaker setups along with propagation effects. The noise map resulting from the calculation represents a **momentary snapshot** based on the given sound system setup, signal spectrum, terrain and meteorology.

**What NoizCalc doesn't do.** NoizCalc does not provide any evaluation of the calculation results nor any indication whether the allowed noise levels in the neighborhood are exceeded. For this kind of assessment, a noise consultant should be involved.

---

## 2. Installation

Administrator rights are required for installation. Double-click the installation file and follow the instructions. Main versions require a new download and installation (e.g. NoizCalc 2.4 and NoizCalc 2.6) but can be installed in parallel. Updates can be done in-app: **Help > Updates & Downloads** (e.g. NoizCalc 2.4 | Update 15.04.2019). Look up the NoizCalc version under **Help > About**.

---

## 3. Calculating the environmental noise

The sound propagation from outdoor stages defined in ArrayCalc to each grid point is calculated according to one of the standards **Nord2000** or **ISO 9613-2**.

The modelling starts with an import of terrain data from Google and noise-relevant objects from OpenStreetMap (OSM) at the desired location. Aside from the topography and buildings, the acoustical ground properties need to be defined with **Ground effects**. The default ground is selectable: "urban" for mostly acoustically hard paved surfaces, or "rural" for countryside or field. Areas with differing properties to the default ground need to be entered. Forests and walls can also be modelled. The automatic import does a lot of the work, but you need to check the created objects and complete the model.

*Figure: Example of the noise propagation of a venue in a landscape situation with forests and partly hard ground in the neighboring village.*

### 3.1 Getting started

#### 3.1.1 NoizCalc reference point in ArrayCalc

Define the position of the NoizCalc reference point in ArrayCalc in the 3D plot. It must be initially activated in **Settings > Advanced features**. Choose a representative point in the middle of the audience area at listening height, typically at front of house.

> **Note:** The SPL in ArrayCalc is **not** used for the calculation in NoizCalc. The desired SPL sum and a (music) spectrum are defined in NoizCalc with the stage. For the calculation, NoizCalc will "drive" the sound system according to the demanded spectrum and SPL so that they are reproduced at the reference point.

#### 3.1.2 Integrate ArrayCalc file

Start NoizCalc and create a new project (New project button or **File > New project**). Clicking **Integrate ArrayCalc file** takes over the general project information (title, description) from a selected ArrayCalc file, and a move/copy dialog integrates the file into your NoizCalc project folder including all defined venues, sources and settings.

#### 3.1.3 Calculation standards

**Nord2000 provides greater accuracy → use ISO 9613-2 when requested.** Calculations with Nord2000 are closer to physical reality — for instance, it uses **Fresnel zones** to calculate ground bounce instead of just a heuristic formula. Furthermore, in ISO 9613-2 the inherent downwind cannot be changed, which leads to a meteorological worst-case scenario.

#### 3.1.4 Default ground

Select the default ground for the entire event area and its surroundings: "urban" for cities and mostly paved, acoustically hard ground, or "rural" for countryside with fields. These values apply where no Ground effect is defined. (Up to NoizCalc 2.4, "rural" is the default ground.)

### 3.2 Modelling

After selecting an object type from the toolbar, enter the coordinates of an object using left mouse clicks, or hold down the left mouse button while drawing a rectangle. See chapter 4.2 "Data entry tools".

> **Note:** Object properties are always taken over from the last object of the same type entered — enter identical or similar objects successively to speed up modelling.

#### 3.2.1 Import of geo data

When switching to the Editor tab in a new project, the **Online map data interface** opens automatically. Search/navigate to the event location and adjust the viewport. For the bitmap, two map data services are available:

- **Google Maps:** aerial image, map, map with terrain. Labels and points of interest can be de-/activated. Tilt view cannot be imported.
- **OpenStreetMap:** map.

Elevation data originates from Google Maps; Buildings, Ground effects and Forests from OpenStreetMap. You can change the default building height (used when no height data is available for import).

Click **Import**. A Digital Ground Model (DGM) is triangulated. Objects are only imported onto a DGM. All data is saved in the project folder. Check the created objects and complete the model; check terrain and buildings in the 3D map view.

The initially imported bitmap is saved separately as `Overview.bmp`. Further bitmaps imported for detailed modelling are named `Detail.bmp` and overwrite each other, whereas the overview bitmap stays available. The background bitmap can be selected in the top-right corner in the editor.

#### 3.2.2 Elevation-relevant objects

Topographic data is not always correct; it can become necessary to change the terrain locally — especially if the ground around the stage is known to be flat but the SUB array is partially "buried" by the DGM. The pre-check before the calculation run gives a warning but does not stop it; usually the result is affected only minimally.

**Elevation points** and **elevation lines** define known elevations. Elevation lines can be **contour lines** (same elevation at every coordinate) or **profile lines** (different elevations). To model a flat venue within a changing topography, delete the elevation points within the venue and add an elevation line around it; the DGM adapts automatically after un-selecting the object. Use **Options > Object types** to hide/display elevation objects in the Editor.

#### 3.2.3 Noise-relevant objects

Objects absorb, reflect or scatter sound waves at sources, along their propagation and at receivers, and are vital for a realistic model.

> **Caution:** The standards have different parameters for Ground effects and Forests. Manually entered objects need to be checked. The import saves both sets.

##### Ground effects

Ground effects define the acoustical ground properties. They affect sound propagation especially near sources and receivers. The default values of "urban" / "rural" apply where no Ground effect is defined — make sure the import did not miss an important area.

**Nord2000 — Impedance class**

| Imp. Class | Ground | Description |
|---|---|---|
| A | very soft | snow, moss-like |
| B | soft | forest floor; short, dense heather-like or thick moss |
| C | uncompacted, loose | turf, grass, loose soil |
| D | normal uncompacted | forest floors, pasture field |
| E | compacted field and gravel | compacted lawns, park area |
| F | compacted dense | gravel road, parking lot |
| G | hard surface | most normal asphalt, concrete |
| H | very hard and dense surface | dense asphalt, concrete, water |

**Nord2000 — Roughness class** (representative *r* = range of heights)

| Roughness class | Range of heights |
|---|---|
| N: Nil | 0 – 0.25 m |
| S: Small | 0.25 m – 0.5 m |
| M: Medium | 0.5 m – 1 m |
| L: Large | 1 m – 2 m |

**ISO 9613-2 — Ground factor G**

| G | Ground | Description |
|---|---|---|
| 0 | hard | paving, concrete, tamped ground, water, ice |
| 1 | porous (soft) | any vegetation, grass, trees, farming land |
| 0 < G < 1 | mixed | fraction of porous to total ground |
| 0.2 | 20 % porous | "urban": 80 % of ground is hard |

> **Note:** Crossing outlines of Ground effects with different values are ambiguous and prevent any calculation. However, NoizCalc can handle a Ground effect defined entirely within another (e.g. a paved area within a park).

##### Forest

The effect of forests on sound propagation depends on the acoustical path length, height and density. Single trees (and lines of trees) have no effect. Shape and height can be defined. With **Nord2000**, mean tree density and mean stem radius can be modified (default Nord2000 values are a best fit to the ISO 9613-2 specification).

Attenuation *A* using path length *d* [m] through the forest:

- **Nord2000:** `A = d · a(f)`, where `a(f)` is calculated from mean tree density, mean stem radius, factor *kp* and mean absorption coefficient.
- **ISO 9613-2:**

  | *d* [m] | *A* [dB] |
  |---|---|
  | < 10 | 0 |
  | 10 … 20 | A₁₀₋₂₀ |
  | 20 … 200 | d · a(f) |
  | ≥ 200 | 200 m · a(f) |

  `A₁₀₋₂₀` and `a(f)` are fixed frequency-dependent values in the standard.

##### Building

A building is described by its outline, its height relative to the ground height (above sea level) and its **reflection loss** (reflection loss = 1 for smooth facades, = 2 for structured facades). Building heights may be estimated by the number of stories/floors. To suppress reflections on a building, deactivate the **Reflections** checkmark.

Activating **Main building** assigns a graphic-plot layout distinguishing "Main building" (e.g. residential) from "auxiliary building" (e.g. garages); define the layout via **Options > Object types**. Corners of buildings form right angles by default (unless right-angle mode is deactivated). When entering buildings, start digitizing with the longest side. Right-angle mode toggle: **F11**.

##### Wall

A wall is described by its base line, its height and its reflection loss on either side. The **screening edge** results from the wall height above the base line; a wall height of 0.0 m already has a screening effect (screening edge = base line). For reflection loss, left/right assignments relate to the data-entry direction.

Standard reflection-loss values (from the German road-noise guideline RLS-90):

| Wall type | Reflection loss |
|---|---|
| Acoustically hard surfaces (concrete, glass) | 1 dB(A) |
| Absorbent walls | 4 dB(A) |
| Highly absorbent walls | 8 – 11 dB(A) |

To exclude a wall from reflecting calculation, deactivate the reflection check boxes for each side. Wall height can change along the wall; coordinates where properties change are marked red in the coordinate list (and by small double circles in the Editor). Click **Constant wall element** to define a height jump at a certain coordinate.

#### 3.2.4 Calculation area

The size of the noise color map (= result) is determined either by the DGM or by defining a calculation area. A smaller area reduces calculation time.

#### 3.2.5 Stage (ArrayCalc file)

A stage contains all loudspeakers (location and orientation in arrays or singularly), electronic filters including ArrayProcessing, and listening planes as defined in ArrayCalc. Select the stage object from the toolbar, place it on the map with a left click, and adjust its orientation.

To select a frequency spectrum, click the library symbol to open the **Emission library**, go to the **System** tab, select a suitable spectrum and click Accept. Enter a meaningful **SPL at the reference point**. A stage can be deactivated for calculations via the **Stage active** checkmark.

**Enter an arbitrary spectrum.** To use an arbitrary spectrum (e.g. from a measurement), open the library, go to the **Values** tab in the Project library, name a new spectrum, and select:
- **Bandwidth:** 3rd-octave or one-octave spectrum
- **Frequency range**
- **Frequency weighting:** A, B, C or D
- **Spectrum type:** sound pressure level or sound power level

Enter the values in the appropriate frequency band and click Accept to assign the spectrum to the stage.

**Calibration of the Stage.** Adjust the NoizCalc reference point in ArrayCalc in the 3D plot (activate it first under **Settings > Advanced features**). NoizCalc uses this reference point to adjust the overall SPL of the sound system — typically at front of house at listening height (e.g. 1.7 m standing audience). Before the actual calculation, the level is calibrated at the reference point. (The reference point must be at least 0.1 m above the terrain — check the z coordinate in the 3D plot in ArrayCalc.)

### 3.3 Calculation

NoizCalc calculates grid noise maps. Click the abacus symbol (or **Calculate > Calculations**), check the calculation settings and start with **Run**.

#### 3.3.1 Calculation settings

- **Number of threads:** how many threads of a multi-core computer are used. By default NoizCalc uses all available threads (cores).
- **Highest reflection order:** how many reflections from obstacles (buildings, walls) are calculated; influences calculation time. According to both standards, the reflection order should be **3**.
- **dB-weighting:** dB(A) or dB(C).
- **Humidity, air pressure, temperature:** environmental parameters important for air absorption; since sound speed is a function of temperature, they influence the wavelength and therefore the screening calculation.

**Nord2000 provides additional meteorology settings:**
- The **Beaufort scale** classifies wind speeds from 0 (calm) to 12 (hurricane). Typically, events are evacuated at wind classes higher than 4 or 5.
- The **Downwind** option enables a hypothetical, non-physical worst-case scenario (commonly used for traffic/industry noise): the calculation includes downwind in **every** direction from source to receiver (like a vortex). For open-air events this is not very fitting due to the short duration; consider specific scenarios instead (e.g. two different wind directions or no wind).
- The **Temperature gradient** defines the vertical temperature change; it affects distant receivers.

The grid noise map is calculated at a specified height above ground with a user-defined equidistant grid distance. Reasonable values lie between **5 m** (calculations including buildings and walls) and **20 m** (free fields). Halving the grid distance quadruples calculation time. **Save as default** changes the presetting permanently.

During grid-noise-map calculation the program shows duration statistics (deactivate via **Calculations > Show grid statistics**). Click **Abort calculation** to stop a running calculation.

#### 3.3.2 Calculation messages

During calculation, the Editor displays messages at the bottom of the screen; errors and warnings appear in red. Double-clicking a message opens and activates the object in question so a definition or geometry error can be fixed easily.

### 3.4 Graphic plot

The calculation result is presented on the **Graphic Plot** tab together with the model geometry and the geometry bitmap. The layout is fixed and contains information on the calculation parameters and calculated stages. Only the sheet size, the background color of the description block, north pointer and length scale, and the color scale are adjustable. Objects can be selected/deselected and their colors changed individually (see "Customizing the Graphic Plot").

---

## 4. In-detail description

Chapter 3 describes the general workflow. Here, formatting and handling are described, plus less frequently used processes such as importing objects and initializing individual bitmaps.

### 4.1 Create and open projects

A NoizCalc project consists of several files stored jointly in one **project folder** — creating a new project creates a folder, not a file. Each folder recognized as a NoizCalc project is displayed with the NoizCalc logo.

Select **File > New project** to create a project folder and enter its name. The project title defaults to the folder name but can be changed. Use project number, engineer and customer fields to add information; the description field may host customer phone numbers or notes. A project can also be initialized via an already-configured ArrayCalc stage (the venue project settings are taken over and the stage is copied in). At startup, the last-used project loads automatically; open another via **File > Open** or the recent list. Right-click for a popup menu with copy, delete or pack.

### 4.2 Data entry tools

#### 4.2.1 General

The Editor manages entry of all noise-relevant data and geometry and prepares it for calculation. Geometry can be prepared on the basis of Google Earth, a scanned and geo-referenced bitmap, or by importing from **DXF, ESRI Shapefile or ASCII** files. Object attributes that might be part of Shape files or ASCII files are **not** taken into account.

NoizCalc works with global Cartesian coordinate systems or a user-defined local coordinate system; the base unit is **meter**. Data using cm, inch or feet must be converted — for DXF import you can enter a conversion factor.

The toolbar contains icons for entering geometry, controls for zooming/editing, and symbols for selecting background graphics. The left block lists the properties and coordinates of the currently active object (for objects without properties, only the coordinate list is shown).

#### 4.2.2 Enter objects

Click the desired object-type icon in the toolbar. With the crosshair cursor active, enter the first coordinate with a left click and the z coordinate in the coordinate table (if a DGM is present, DGM elevations are auto-entered). For line/area objects, enter additional coordinates. End line/area entry by double-clicking or the **Finish object (F2)** icon. For right-angle buildings, close with a double-click on the third coordinate (the fourth is added automatically). North-oriented rectangular objects can be entered by drawing a frame. Coordinates can also be typed/corrected in the coordinate list. To enter objects, the mode "Select object or create new object" must be active.

#### 4.2.3 Edit objects and object properties

Enter objects by clicking with the left mouse button. The crosshair cursor is visible when entering objects; near another object it changes into the selection cursor and displays the object's coordinates and properties on the right.

Notes:
- To digitize new coordinates when objects are too close and the cursor changes, disable the selection cursor via **Disable select mode**.
- If objects are stacked and the wrong one activates, click the desired object-type icon in the toolbar (the active object type is found first).
- Use the navigation bar above the properties to move object-to-object within the selected type.
- Pressing **ESC** while digitizing a new (unfinished) object deletes it.
- To place an object directly on the edge of another (e.g. terraced houses), disable the selection cursor — the cursor becomes a capture circuit that snaps the edge/corner of the previous object.

The **Overview** icon zooms out to display all entered objects. The "hand" icon moves the data; the "magnifying glass" zooms with left/right click. The mouse wheel also zooms (turn) and moves (hold); Ctrl rotates. If "center to current object" is checked, the geometry moves so the object is centered.

##### Selection of multiple objects

Ctrl + left click selects multiple objects; alternatively draw a frame with the right mouse button (all objects with at least one coordinate inside are marked). Marked objects can be deleted jointly (right-click menu or Ctrl+Del), moved (drag the pink diamond), or rotated (Ctrl + left click the pink diamond). Shift + right-button frame selects only objects of the current type; Shift+Ctrl adds more of that type. Multiple objects of the same type can have properties modified jointly — except objects that change properties within themselves (road, railway, wall or berm).

##### Object operations

Right-click an object for menu options:
- **Move object(s):** drag the pink diamond with the left mouse button.
- **Rotate object(s):** hold Ctrl and drag the pink diamond.
- Moved/rotated objects are automatically re-referenced to the DGM (if the DGM is deactivated, the z coordinate is not updated).
- **Duplicate object:** duplicated objects appear with a slight offset and can be dragged.
- **Delete:** popup menu or Ctrl+Del; single coordinates via right-click → **Delete current point** (or the Del key in the coordinate list).
- **Append points:** active for line-type objects (add coordinates at the end).
- **Insert/move points:** move single coordinates or insert at the cursor.
- **Divide line objects:** right-click at the split coordinate → **Divide line at the current point** (each part needs more than 2 coordinates).
- **Distance indicator:** distance between the current cursor position and the last digitized coordinate is shown in the status bar in [m].

#### 4.2.4 Initialization of background bitmaps

Digitizing on top of scanned background graphics is the most common data-entry mode. Use any number of background bitmaps in BMP, JPG, PNG or TIF (keep resolution and color depth moderate). A geo-reference must be established between the pixel bitmap and the world/local coordinate system; if a geo-reference transformation ships with the bitmap, NoizCalc reads it automatically.

Open **Edit > Initialize bitmap** and open the scanned map. Reference points should be as far apart as possible and enclose the investigation area. Enter the first reference point's world coordinates, then left-click the graphics to assign its bitmap location (the first click opens an enlargement for precise positioning). Define the second reference point and click OK. In the Editor, switch between background bitmaps via the selection list; the click box toggles display.

#### 4.2.5 Import

Model geometry can be imported in **DXF, ESRI Shapefile or ASCII** formats. Objects must be organized in individual files (Shapefiles) or layers (DXF). Select the target object type, optionally assign default properties, then **File > Import** and the required format. For DXF, an extra window selects layers (Shift/Ctrl to mark multiple layers of the same object type; layer assignment is lost). A conversion factor can be entered for DXF drawings in mm or feet.

**Filter elevation points prior to import.** Elevation spot heights imported as ASCII or DXF are often a grid or cloud with very small spacing (e.g. 1×1 m). Since the number of elevation coordinates strongly affects DGM size and calculation, filtering is useful. In the import dialog, the number of points and structure (grid or cloud) is shown. To keep all spot heights 1:1, click Import; to filter, enter the maximum allowed deviation [m] between the import data and the resulting elevation model (preset 40 cm → ± 0.40 m tolerance). Click the green arrow to filter and import; the result shows how many coordinates (and what percentage) remain.

### 4.3 Digital Ground Model

A **Digital Ground Model (DGM)** is the basis for a 3D noise model, generated from elevation lines and elevation points; NoizCalc calculates it automatically. Entered/imported objects are automatically placed on the triangulated surface, and moved objects adjust to the new-location elevation. Terrain elevation can be assigned manually via **Edit > Set current object on DGM** or **Edit > Set all objects of current object type on DGM**. Check the model in the 3D map.

### 4.4 Display types: Site map, vertical map, 3D map

Geometry data is entered in the **site map**. Review data-model integrity in the **3D map** or the projection in the **vertical map** (selection list; F10 toggles site/3D). Object properties can be edited in all display types if selection mode is active. Mouse wheel: move (hold), zoom (rotate), change distance (Shift+move), rotate/tilt world or light (Ctrl+move). The right mouse button in "hand mode" selects movements/view modifications; **Move** is initially active. In **Change light** mode, drag with the left button to move the light source. For **Drawing type** choose wire-frame vs hidden lines/surfaces; for **Projection type** choose orthogonal vs perspective. DGM presentation in the 3D map can be toggled (its color is set in the "Elevation line" object type). A background geometry bitmap can also be displayed in the 3D map (activate the 3D check box in the menu bar and in the Geometry bitmap object type).

#### 4.4.1 Further settings

In top view, presentation can show area fillings (filled or line-only) — suppressing fill helps when digitizing over a background bitmap.

#### 4.4.2 Coordinate list

The coordinate list shows x/y coordinates and (depending on object type) the base or relative height, all in meters. Terrain elevation is display-only (either 0 or derived from the DGM). You can: correct x/y; insert coordinates (right-click → **Insert points**, interpolated at the midpoint); delete coordinates (Del); delete property changes within roads/railways/walls/berms (right-click → **Delete properties**). A line reduced to a single coordinate (or an area to 2) is deleted entirely.

### 4.5 Additional objects

Click the icon for the desired object and select coordinates with the left mouse button.

#### 4.5.1 Text

Texts write descriptions/annotations for the graphical output (e.g. source name, house number), linked to the entry coordinate.

#### 4.5.2 General line

You can, e.g., import all lines from a DXF file to the **General line** object type and digitize required objects similarly to digitizing from a background bitmap (deactivate "Select objects" when digitizing). Afterwards, hide lines via **Options > Object types** (deselect Show in editor) or use them as background data in the graphics plot.

### 4.6 Customizing the graphic plot

#### 4.6.1 Set sheet size

Before determining the map viewport, select the sheet size so the program knows the active drawing area and can determine the length scale. Open **Graphics > Sheet settings** (or click the white sheet frame). Select sheet size and the description-block background color; access elements on the respective tabs or by clicking the element.

#### 4.6.2 Set viewport and length scale

By default the map is scaled to present the entire grid noise map area. Use the right mouse button or the graphics menu to select the viewport. Click the scale bar (or **Graphics > Edit map viewport**) to numerically set scale, rotation and center coordinates and to format the north arrow. The length scale adjusts automatically when zooming; rotating the map also rotates the north arrow and background graphics. Scale can be shown in meters or feet. Ratio 1:1000 = 1 m represents 1000 m (no mixing of units). Move the north arrow by dragging with the left mouse button.

#### 4.6.3 Edit legend, format objects

Open **Options > Object types** (or click the legend) and select the object type to customize. The layout of all objects is defined in the object-type file (geometry objects, the grid noise map object type, geometry bitmaps); default definitions ship with the program and can be customized. Per object type, set visibility in the Editor and graphics display, legend text, and draw sequence (higher = drawn later, may hide lower-sequence objects).

- **Point type object types:** define size in [mm] (plan size, scale-independent) or [m] (world coordinates, scale-dependent). Symbol size uses an imaginary rectangle around the symbol (longest side determines size). The **Symbol** button selects a different symbol; also set symbol line width and border lines.
- **Line type object types:** enter line width and line color.
- **Area type objects:** define fill color, border, and hatch pattern separately. *Hatch pattern:* use the double arrow to select and confirm; if **Fit to first line** is checked, the pattern is parallel to a line through the first 2 coordinates.

#### 4.6.4 Texts

Text layout is set in the editor; all graphics settings (color, font) come from the Editor definitions. In object types, adjust all text sizes via **Size as factor** of the Editor-selected sizes.

#### 4.6.5 Grid Noise Map

The Grid Noise Map shows noise contour areas filled with a user-defined color scale; contour lines and color scale can be edited.

**Edit the appearance of contour lines.** Go to **Options > Object types** and select the grid noise map object type.
- **Show contour lines** should always be "show" except when presenting the grid cells (deactivating draws no contour lines/fills, even for the main interval).
- **Filter bandwidth [m]** defines a bandwidth within which contour-controlling points are deleted, smoothing the lines.
- **Bezier type** defines contour-line form accuracy. *Exact* Bezier curves pass through the calculated interception points; *smooth* Bezier only uses them to pull the curve (line less accurate under strong/changing curvature).
- **Edge line** displays the calculation-area border (set color and line width).
- **Definition of intervals:** **Fill contours** fills areas between contour lines per the scale colors; contours can also be shown as lines with chosen color/width; you can disable Fill contours and draw the contour lines in scale colors.

**Edit the colored scale.** When the grid noise map loads, the program determines min/max levels and suggests a sensible scale. Colors come from the color palette. The scale unit follows the dB weighting (dB(A)/dB(C)); "<su:>" is a variable for the scale unit in the color-scale title. Click the color scale in the description block or **Graphics > Edit color scale**. Enter the smallest interval value, the interval magnitude (in dB(A)) and the number of intervals; **Ascending** defines whether the lowest levels are at top or bottom. Color selection uses a palette (16 color fields per line). If fewer than 16 intervals are used, NoizCalc selects a color sequence from 16 colors; select **Keep color sequence** to disable automatic assignment.

*Manually modify scale intervals:* interval size is preset linearly from the noise values, but can be customized (interval size or scale colors) in the Edit scale dialog. In the **Value** column you can change the upper interval limit; interval sizes need not be constant.

**Transparent grid maps.** If a geometry bitmap is in the Editor background, the grid map can be displayed on it transparently or shaded (provided the bitmap is larger than the calculation area). Contour lines are drawn on top of the bitmap in the selected or scale color. In object types (click the legend), select "transparent" or "shade" and enter a percentage for both. Optionally take only bitmap gray values into account (**Bitmap area to gray**). *Transparent* suits fully colored darker bitmaps (aerial photos); *Shaded* suits digital base maps with lines and bright colors. If result colors are too distorted, use **Brighten** (aerial photos) or **Contrast** (base maps) in the geometry bitmap object type.

> **Note:** In the background, the geometry bitmap is set to "normal" and the contour-line output sequence is set higher than the grid noise map's.

#### 4.6.6 Color palette

The color palette defines object/element colors and scale colors (16 consecutive colors are used as scale colors). Open **Options > Colors**; values are RGB (Red-Green-Blue components, 0–255). The extended Colors dialog opens automatically when clicking a non-favorite color field.

- **Define colors / compile scale colors:** click "extended"; enter RGB values, or drag-and-drop an existing color to the extended field and modify it, then drop the new color to the desired position. Compose color sequences by moving colors into free (black) matrix fields.
- **Interpolate colors:** to generate gradients, place the first color in an empty black field and the second to its right, leaving black spaces for interpolation; click the calculator icon to fill the gap. For a gray scale, the first value cannot be pure black (0,0,0) — use a very dark gray (e.g. 5,5,5).
- **Set colors to black:** click the "set black" icon (active while the left button is held) to quickly erase unwanted color favorites.
- **Print color values:** print the entire palette with RGB values (printed colors depend on saturation, resolution and paper).

#### 4.6.7 Export and print graphic plot

Click the **Print** icon to print the map or save it as a PDF. Save graphics sheets as bitmaps or Windows Meta files using the **Save as** icon (the on-screen sheet is exported).

---

## 5. Troubleshooting

If map data import is not successful, change the coordinate settings to the southern hemisphere and repeat the import: **Options > Coordinate settings > UTM coordinates (southern hemisphere)**.

---

## 6. Legal notice

**Trademarks.** NoizCalc and the NoizCalc logo are trademarks of d&b audiotechnik GmbH & Co. KG and SoundPLAN GmbH. All other trademarks, brand names, etc. used in this manual are the property of their respective owners and are subject to the laws of different countries.

d&b audiotechnik GmbH & Co. KG, Eugen-Adolff-Straße 134, D-71522 Backnang, Germany. Phone +49-7191-9669-0, Fax +49-7191-950000.

10/2019 © d&b audiotechnik GmbH & Co. KG + SoundPLAN GmbH
