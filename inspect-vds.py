from dataclasses import dataclass
import numpy as np
import numpy.typing as npt
from numpy.typing import NDArray
from typing import Tuple, Optional, List
import os
from pathlib import Path
from enum import Enum
import matplotlib.pyplot as plt
from PIL import Image

import time

import openvds  # type: ignore
from openvds import VolumeDataLayoutDescriptor as VDLayoutDesc  # type: ignore
from openvds import VolumeDataChannelDescriptor as VDChannelDesc  # type: ignore
from openvds import VolumeDataAxisDescriptor as VDAxisDesc  # type: ignore
from openvds import (
    CompressionMethod,  # type: ignore
    MetadataContainer,  # type: ignore
    KnownUnitNames,  # type: ignore
    getLayout,  # type: ignore
    getAccessManager,
    DimensionsND,  # type: ignore
    IVolumeDataAccessManager,  # type: ignore
)

import typer

app = typer.Typer()

@dataclass
class SeismicAxis:
    min_val: int
    max_val: int
    step: int

    @staticmethod
    def from_array(arr: npt.NDArray[np.int32]) -> 'SeismicAxis':
        return SeismicAxis(
            min_val=int(np.min(arr)),
            max_val=int(np.max(arr)),
            step=int(arr[1] - arr[0])
        )

    def as_array(self) -> npt.NDArray[np.int32]:
        """Generate a NumPy array from the axis range and step."""

        return np.array(np.arange(self.min_val, self.max_val + self.step, self.step, dtype=np.int32))

    def __len__(self) -> int:
        """Return the number of steps between min_val and max_val."""
        return (self.max_val - self.min_val) // self.step + 1

    def __str__(self) -> str:
        """Provide a nice string representation of the SeismicAxis object."""
        length = len(self)
        return f"SeismicAxis(min_val={self.min_val}, max_val={self.max_val}, step={self.step}, length={length})"


@dataclass
class SeismicCube:
    data: npt.NDArray[np.float32]
    iline_axis: SeismicAxis
    xline_axis: SeismicAxis
    sample_axis: SeismicAxis

    def __str__(self):
        return f"SeismicCube(\ndata.shape={self.data.shape}\n{self.iline_axis}\n{self.xline_axis}\n{self.sample_axis}\n)"


class CompressionOVDS(Enum):
    NONE = 0
    WAVELET = 1
    RLE = 2
    ZIP = 3
    WAVELET_NORMALIZE_BLOCK = 4
    WAVELET_LOSSLESS = 5
    WAVELET_NORMALIZE_BLOCK_LOSSLESS = 6


def read_cube_from_vds(
        path: Path, account_name:str="", container_name:str="", account_key_env_var:str="") -> SeismicCube:
    is_local = (account_name=="")
    vds_url = get_vds_url(is_local, container_name, str(path))
    connection_string = get_vds_connection_string(
        is_local, account_name, account_key_env_var
    )

    vds_handler = openvds.open(vds_url, connection_string)  # type: ignore
    manager = openvds.getAccessManager(vds_handler)
    layout = getLayout(vds_handler)

    inlines, axis1_name = vds_axis_descriptor_to_array(layout, 2)
    xlines, axis2_name = vds_axis_descriptor_to_array(layout, 1)
    samples, axis3_name = vds_axis_descriptor_to_array(layout, 0)

    print(axis1_name, axis2_name, axis3_name)

    shape = (
        layout.getDimensionNumSamples(0),
        layout.getDimensionNumSamples(1),
        layout.getDimensionNumSamples(2),
    )
    layout

    min = (0, 0, 0)
    req = manager.requestVolumeSubset(min, shape, format=VDChannelDesc.Format.Format_R32)  # type: ignore
    data = req.data

    if data is None:
        raise ValueError(f"Data reading from {path} is None")

    data = data.reshape((shape[2], shape[1], shape[0])).T


    seismic_cube = SeismicCube(
        data = data,
        iline_axis=SeismicAxis.from_array(inlines),
        xline_axis=SeismicAxis.from_array(xlines),
        sample_axis=SeismicAxis.from_array(samples)

    )
    return seismic_cube


def get_vds_connection_string(
    is_local: bool, account_name: str = "", account_key_env_var: str = ""
) -> str:
    conn_string = ""
    if is_local:
        conn_string = ""
    else:
        conn_string = get_azure_connection_string(
            account_name=account_name, account_key_env_var=account_key_env_var
        )
    return conn_string


def get_azure_connection_string(account_name: str, account_key_env_var: str) -> str:
    account_key = os.environ.get(account_key_env_var)
    if not account_key:
        raise RuntimeError(f"Could not find account key with env var {account_key_env_var}")
    conn_str = (
        "DefaultEndpointsProtocol=https;"
        + f"AccountName={account_name};"
        + f"AccountKey={account_key};"
        + "EndpointSuffix=core.windows.net"
    )
    return conn_str


def get_azure_url_vds_format(container_name: str, vds_path: str) -> str:
    return f"azure://{container_name}/{vds_path}"


def get_vds_url(is_local: bool, container_name: str = "", vds_path: str = "") -> str:
    if is_local:
        return vds_path
    else:
        return get_azure_url_vds_format(container_name=container_name, vds_path=vds_path)


def vds_axis_descriptor_to_array(layout: VDLayoutDesc, axis: int) -> Tuple[NDArray[np.int32], str]:
    axis_desc = layout.getAxisDescriptor(axis)
    min = int(axis_desc.coordinateMin)
    max = int(axis_desc.coordinateMax)
    name = axis_desc.getName()
    num = axis_desc.getNumSamples()
    return np.linspace(min, max, num, dtype=np.int32), name

def plot_cube_planes(cube: np.ndarray, output_path: str) -> None:
    fig, axes = plt.subplots(3, 3, figsize=(15, 15))

    # Indices for the middle planes
    mid_indices = [dim_size // 2 for dim_size in cube.shape]

    # Titles for the subplots
    titles = ['X=0', 'X=Middle', 'X=Max', 'Y=0', 'Y=Middle', 'Y=Max', 'Z=0', 'Z=Middle', 'Z=Max']

    # Plotting
    axes[0, 0].imshow(cube[0, :, :])
    axes[0, 1].imshow(cube[mid_indices[0], :, :])
    axes[0, 2].imshow(cube[-1, :, :])
    axes[1, 0].imshow(cube[:, 0, :])
    axes[1, 1].imshow(cube[:, mid_indices[1], :])
    axes[1, 2].imshow(cube[:, -1, :])
    axes[2, 0].imshow(cube[:, :, 0])
    axes[2, 1].imshow(cube[:, :, mid_indices[2]])
    axes[2, 2].imshow(cube[:, :, -1])

    for ax, title in zip(axes.flat, titles):
        ax.set_title(title)

    # Removing x and y ticks
    for ax in axes.flat:
        ax.set_xticks([])
        ax.set_yticks([])

    plt.tight_layout()
    plt.savefig(output_path)


    
def normalize_array(arr: np.ndarray) -> np.ndarray:
    """Normalize a numpy array to the range 0-255."""
    arr_min = np.min(arr)
    arr_max = np.max(arr)
    return ((arr - arr_min) / (arr_max - arr_min) * 255).astype('uint8')

def plot_cube_planes_to_individual_files_pillow(cube: np.ndarray, output_dir: str) -> None:
    # Ensure the output directory exists
    os.makedirs(output_dir, exist_ok=True)

    # Indices for the middle planes
    mid_indices = [dim_size // 2 for dim_size in cube.shape]

    # Titles for the files
    titles = ['X=0', 'X=Middle', 'X=Max', 'Y=0', 'Y=Middle', 'Y=Max', 'Z=0', 'Z=Middle', 'Z=Max']

    # Planes to plot
    planes = [
        (cube[0, :, :], titles[0]),
        (cube[mid_indices[0], :, :], titles[1]),
        (cube[-1, :, :], titles[2]),
        (cube[:, 0, :], titles[3]),
        (cube[:, mid_indices[1], :], titles[4]),
        (cube[:, -1, :], titles[5]),
        (cube[:, :, 0], titles[6]),
        (cube[:, :, mid_indices[2]], titles[7]),
        (cube[:, :, -1], titles[8]),
    ]

    # Plotting and saving each plane as an individual image
    for plane, title in planes:
        # Normalize the array for image saving
        normalized_plane = normalize_array(plane)
        img = Image.fromarray(normalized_plane)
        file_path = os.path.join(output_dir, f'{title.replace("=", "").replace(" ", "_")}.png')
        img.save(file_path)

@app.command()
def meta(file_path: str, save_png=False, out_dir=""):
    before_read = time.time()
    cube = read_cube_from_vds(Path(file_path))
    dur_ms = (time.time() - before_read)*1000
    print(f"Reading took {dur_ms} ms")
    print(cube)
    if not save_png:
        plot_cube_planes(cube.data, "sliced_cube.png")
    else:
        plot_cube_planes_to_individual_files_pillow(cube.data, out_dir)


if __name__ == '__main__':
    app()
